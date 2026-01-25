use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::json;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Mutex};
use tokio_tungstenite::tungstenite::Message as WsMessage;
use tokio_tungstenite::{connect_async, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};

use crate::config::ExchangeConfig;
use crate::domain::{Orderbook, PriceLevel};

/// Poloniex WebSocket URL.
const WEBSOCKET_URL: &str = "wss://ws.poloniex.com/ws/public";

/// Default interval to send ping messages.
/// Poloniex requires a message or ping every 30 seconds.
/// Using 20 seconds for reliability margin.
const DEFAULT_PING_INTERVAL: Duration = Duration::from_secs(20);

/// Default delay before reconnecting.
const DEFAULT_RECONNECT_DELAY: Duration = Duration::from_secs(5);

/// Default orderbook depth.
const DEFAULT_DEPTH: u8 = 10;

/// Callback type for reconnection failure events.
type OnReconnectFailed = Box<dyn Fn(&str, &str) + Send + Sync>;

/// WebSocket configuration for Poloniex exchange.
struct WebSocketConfig {
    /// WebSocket server URL.
    url: String,
    /// List of trading pairs to subscribe (e.g., "BTC_USDT").
    pairs: Vec<String>,
    /// Orderbook depth (number of price levels). Poloniex supports: 5, 10, 20.
    depth: u8,
    /// Interval between ping messages.
    ping_interval: Duration,
    /// Delay before attempting reconnection.
    reconnect_delay: Duration,
    /// Callback when reconnection fails permanently.
    on_reconnect_failed: Option<OnReconnectFailed>,
}

impl WebSocketConfig {
    /// Creates a new WebSocketConfig from ExchangeConfig.
    fn from_config(config: &ExchangeConfig, pairs: Vec<String>) -> Self {
        let (ping_interval, reconnect_delay) = config
            .websocket
            .as_ref()
            .map(|ws| (ws.ping_interval, ws.reconnect_delay))
            .unwrap_or((DEFAULT_PING_INTERVAL, DEFAULT_RECONNECT_DELAY));

        Self {
            url: WEBSOCKET_URL.to_string(),
            pairs,
            depth: DEFAULT_DEPTH,
            ping_interval,
            reconnect_delay,
            on_reconnect_failed: None,
        }
    }
}

/// Type alias for WebSocket connection.
type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;
type WsSink = SplitSink<WsStream, WsMessage>;
type WsSource = SplitStream<WsStream>;

/// WebSocket error type.
type WsError = tokio_tungstenite::tungstenite::Error;

/// WebSocket manager for Poloniex exchange.
pub struct WebSocketManager {
    config: WebSocketConfig,
    sink: Arc<Mutex<Option<WsSink>>>,
    orderbooks_tx: mpsc::UnboundedSender<Orderbook>,
    closed: Arc<AtomicBool>,
}

impl WebSocketManager {
    /// Creates a new WebSocket manager.
    pub fn new(exchange_config: &ExchangeConfig, pairs: Vec<String>) -> (Self, mpsc::UnboundedReceiver<Orderbook>) {
        let config = WebSocketConfig::from_config(exchange_config, pairs);
        let (orderbooks_tx, orderbooks_rx) = mpsc::unbounded_channel();

        let manager = Self {
            config,
            sink: Arc::new(Mutex::new(None)),
            orderbooks_tx,
            closed: Arc::new(AtomicBool::new(false)),
        };

        (manager, orderbooks_rx)
    }

    /// Returns true if the manager is closed.
    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Marks the manager as closed.
    fn mark_closed(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    /// Connects to WebSocket server.
    /// Returns the read half of the stream for message processing.
    async fn connect(&self) -> Result<WsSource, WsError> {
        info!(url = %self.config.url, "connecting to websocket");

        let (ws_stream, _response) = connect_async(&self.config.url).await.map_err(|e| {
            error!(error = %e, url = %self.config.url, "failed to connect to websocket");
            e
        })?;

        let (sink, stream) = ws_stream.split();
        *self.sink.lock().await = Some(sink);

        info!("websocket connected");

        Ok(stream)
    }

    /// Closes the WebSocket connection.
    pub async fn close(&self) {
        if self.is_closed() {
            return;
        }
        self.mark_closed();

        let mut guard = self.sink.lock().await;
        if let Some(mut sink) = guard.take() {
            if let Err(e) = sink.close().await {
                error!(error = %e, "failed to close websocket");
            }
        }

        info!("websocket closed");
    }

    /// Attempts to reconnect to WebSocket after a delay.
    /// Returns the new read stream on success.
    async fn reconnect(&self) -> Result<WsSource, WsError> {
        // Close existing connection if any
        {
            let mut guard = self.sink.lock().await;
            if let Some(mut sink) = guard.take() {
                let _ = sink.close().await;
            }
        }

        // Check if the manager was closed
        if self.is_closed() {
            return Err(tokio_tungstenite::tungstenite::Error::AlreadyClosed);
        }

        info!(delay = ?self.config.reconnect_delay, "reconnecting");

        // Wait before reconnecting
        tokio::time::sleep(self.config.reconnect_delay).await;

        // Check again after sleep
        if self.is_closed() {
            return Err(tokio_tungstenite::tungstenite::Error::AlreadyClosed);
        }

        // Reconnect and resubscribe
        let stream = self.connect().await?;
        self.send_subscribe_message().await?;

        Ok(stream)
    }

    /// Attempts to reconnect and returns the new stream on success.
    /// Logs error and invokes callback on failure.
    async fn try_reconnect(&self) -> Option<WsSource> {
        match self.reconnect().await {
            Ok(new_stream) => Some(new_stream),
            Err(e) => {
                error!(error = %e, "reconnect failed");
                if let Some(ref callback) = self.config.on_reconnect_failed {
                    callback("poloniex", &e.to_string());
                }
                None
            }
        }
    }

    /// Subscribes to orderbook updates: connects, sends subscription, and spawns read/ping loops.
    /// Runs until closed or error.
    pub async fn subscribe(&self) -> Result<(), WsError> {
        // 1. Connect to WebSocket
        let stream = self.connect().await?;

        // 2. Send a subscription message
        self.send_subscribe_message().await?;

        // 3. Spawn ping loop
        let _ping_handle = self.spawn_ping_loop();

        // 4. Run read loop (blocks until closed or error)
        self.read_loop(stream).await;

        Ok(())
    }

    /// Sends the subscription message to the WebSocket.
    async fn send_subscribe_message(&self) -> Result<(), WsError> {
        let mut guard = self.sink.lock().await;
        let sink = guard.as_mut().ok_or_else(|| {
            tokio_tungstenite::tungstenite::Error::AlreadyClosed
        })?;

        // Normalize depth to supported values: 5, 10, 20
        let depth = normalize_depth(self.config.depth);

        // Convert pairs to Poloniex symbol format (BTC/USDT -> BTC_USDT)
        let symbols: Vec<String> = self.config
            .pairs
            .iter()
            .map(|p| pair_to_symbol(p))
            .collect();

        // Poloniex subscription format:
        // {"event": "subscribe", "channel": ["book"], "symbols": ["BTC_USDT"], "depth": 10}
        let sub_msg = json!({
            "event": "subscribe",
            "channel": ["book"],
            "symbols": symbols,
            "depth": depth
        });

        let msg = WsMessage::Text(sub_msg.to_string().into());
        sink.send(msg).await.map_err(|e| {
            error!(error = %e, "failed to subscribe");
            e
        })?;

        info!(symbols = ?symbols, depth = depth, "subscribed to orderbook");

        Ok(())
    }

    /// Continuously reads messages from WebSocket and sends orderbook updates.
    /// Automatically reconnects on connection errors.
    /// Returns when the manager is closed or reconnection fails permanently.
    async fn read_loop(&self, mut stream: WsSource) {
        loop {
            if self.is_closed() {
                break;
            }

            tokio::select! {
                msg = stream.next() => {
                    match msg {
                        Some(Ok(WsMessage::Text(text))) => {
                            if let Some(orderbook) = parse_message(&text) {
                                if self.orderbooks_tx.send(orderbook).is_err() {
                                    warn!("orderbook channel closed");
                                    break;
                                }
                            }
                        }
                        Some(Ok(WsMessage::Close(_))) => {
                            info!("websocket closed by server");
                            match self.try_reconnect().await {
                                Some(new_stream) => stream = new_stream,
                                None => break,
                            }
                        }
                        Some(Ok(_)) => {
                            // Ignore other message types (Ping, Pong, Binary)
                        }
                        Some(Err(e)) => {
                            if should_reconnect(&e) {
                                error!(error = %e, "websocket error, attempting reconnect");
                                match self.try_reconnect().await {
                                    Some(new_stream) => stream = new_stream,
                                    None => break,
                                }
                            } else {
                                error!(error = %e, "websocket error (non-recoverable)");
                                break;
                            }
                        }
                        None => {
                            info!("websocket stream ended");
                            break;
                        }
                    }
                }
            }
        }

        // Cleanup
        let mut guard = self.sink.lock().await;
        if let Some(mut sink) = guard.take() {
            let _ = sink.close().await;
        }
    }

    /// Spawns the ping loop as a background task.
    /// Returns a JoinHandle that completes when the loop exits.
    /// Can be called before moving self into read_loop.
    pub fn spawn_ping_loop(&self) -> tokio::task::JoinHandle<()> {
        let ping_interval = self.config.ping_interval;
        let sink = Arc::clone(&self.sink);
        let closed = Arc::clone(&self.closed);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(ping_interval);

            loop {
                interval.tick().await;

                if closed.load(Ordering::SeqCst) {
                    break;
                }

                let mut guard = sink.lock().await;
                let Some(sink_ref) = guard.as_mut() else {
                    break;
                };

                // Poloniex uses JSON ping: {"event": "ping"}
                let ping_msg = json!({"event": "ping"});
                let msg = WsMessage::Text(ping_msg.to_string().into());

                if let Err(e) = sink_ref.send(msg).await {
                    warn!(error = %e, "ping failed");
                } else {
                    debug!("ping sent");
                }
            }
        })
    }
}

/// Returns true if the error warrants a reconnection attempt.
fn should_reconnect(error: &WsError) -> bool {
    use tokio_tungstenite::tungstenite::Error;
    matches!(
        error,
        Error::ConnectionClosed
            | Error::AlreadyClosed
            | Error::Io(_)
            | Error::Tls(_)
            | Error::Http(_)
    )
}

/// Normalizes depth to Poloniex supported values: 5, 10, 20.
fn normalize_depth(depth: u8) -> u8 {
    if depth <= 5 {
        5
    } else if depth <= 10 {
        10
    } else {
        20
    }
}

/// Converts "BTC/USDT" to "BTC_USDT".
fn pair_to_symbol(pair: &str) -> String {
    pair.replace('/', "_")
}

/// Converts "BTC_USDT" to "BTC/USDT".
fn symbol_to_pair(symbol: &str) -> String {
    symbol.replace('_', "/")
}

/// Poloniex orderbook WebSocket message.
/// Format: {"channel":"book","data":[{"symbol":"BTC_USDT","createTime":123,"asks":[...],"bids":[...],"id":1,"ts":123}]}
#[derive(Debug, Deserialize)]
struct OrderbookMessage {
    channel: Option<String>,
    event: Option<String>,
    data: Option<Vec<OrderbookData>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct OrderbookData {
    symbol: String,
    create_time: Option<i64>,
    asks: Vec<Vec<String>>,
    bids: Vec<Vec<String>>,
    #[allow(dead_code)]
    id: i64,
    ts: Option<i64>,
}

/// Parses a WebSocket message into an Orderbook.
/// Returns None for non-orderbook messages (pong, subscribe confirmation, etc.)
fn parse_message(data: &str) -> Option<Orderbook> {
    let msg: OrderbookMessage = serde_json::from_str(data).ok()?;

    // Check if it's an orderbook message
    if msg.channel.as_deref() != Some("book") {
        debug!(channel = ?msg.channel, "not an orderbook message");
        return None;
    }

    // Skip subscription confirmation or pong
    if let Some(event) = &msg.event {
        if event == "subscribe" || event == "pong" {
            debug!(event = %event, "control message");
            return None;
        }
    }

    let data = msg.data?.into_iter().next()?;

    let timestamp = data.ts.or(data.create_time).unwrap_or(0);
    let system_time = UNIX_EPOCH + Duration::from_millis(timestamp as u64);

    let bids = parse_levels(&data.bids);
    let asks = parse_levels(&data.asks);

    Some(Orderbook {
        exchange: "poloniex".to_string(),
        pair: symbol_to_pair(&data.symbol),
        bids,
        asks,
        timestamp: system_time,
    })
}

/// Parses price levels from raw string arrays.
fn parse_levels(levels: &[Vec<String>]) -> Vec<PriceLevel> {
    levels
        .iter()
        .filter_map(|level| {
            if level.len() < 2 {
                return None;
            }
            let price = Decimal::from_str(&level[0]).ok()?;
            let quantity = Decimal::from_str(&level[1]).ok()?;
            // Skip zero quantity (removed level)
            if quantity.is_zero() {
                return None;
            }
            Some(PriceLevel { price, quantity })
        })
        .collect()
}