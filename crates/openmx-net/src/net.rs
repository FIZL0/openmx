//! The transport and the lobby protocol (opentennis's `hst-base/src/net.rs`, M2f, cut to a ride's needs).
//!
//! Peers are iroh endpoints: QUIC, encrypted, hole punching, and a relay for when punching fails (n0's public
//! relays, your own `iroh-relay`, or none on a LAN). The lobby is a star: every guest has one connection, to the
//! host, and the host relays whatever goes between guests (the ride's messages and datagrams), so a guest that
//! can reach the host can play with everyone.
//!
//! The protocol (ALPN [`ALPN`]): the joiner opens a stream and sends [`Msg::Hello`] (build, its local players'
//! picks, its player's name); the host answers [`Msg::Room`] (seated) or [`Msg::Refused`], and sends `Room` again
//! on every change (a guest's [`Msg::Prepared`] marks it ready). The host's Start, once every guest is ready, sends
//! [`Msg::Start`] with the seed; each guest answers its [`Handshake`] for that line-up ([`Msg::Shake`]), and the host
//! refuses any that differs from its own (the peer is let go with why, the rest go back to the lobby) or, when all
//! agree, sends [`Msg::Go`]: the ride starts on every peer with the host's seed. A peer that drops mid-ride sends
//! everyone left back to the lobby ([`Msg::Lobby`]). Messages are JSON, a u32 length before each; datagrams (the
//! ride's unreliable traffic) carry the peer they are for or from.
//!
//! The app talks to this through [`Net`]: [`Cmd`]s in, [`Event`]s out; the network runs on its own tokio thread.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, mpsc};

#[cfg(not(target_arch = "wasm32"))]
use iroh::address_lookup::AddrFilter;
use iroh::endpoint::{Connection, RecvStream, SendStream, presets};
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayMode, RelayUrl, SecretKey};
#[cfg(not(target_arch = "wasm32"))]
use iroh_mainline_address_lookup::DhtAddressLookup;
#[cfg(not(target_arch = "wasm32"))]
use iroh_mdns_address_lookup::MdnsAddressLookup;
use iroh_tickets::endpoint::EndpointTicket;
#[cfg(not(target_arch = "wasm32"))]
use n0_mainline::Dht;
// tokio's natively, the page's clock and timers in a browser (N4)
use n0_future::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc as amp;

#[cfg(target_arch = "wasm32")]
use crate::board::MdnsAddressLookup;
use crate::board::{self, Lan};
#[cfg(target_arch = "wasm32")]
use crate::codes::Dht;
use crate::codes::{self, Codes};
use crate::handshake::{self, Disc, Handshake};
use crate::room::{HOST, Listing, Pick, Room, quick_pick};

/// The lobby's protocol. 2 since N6: the ride's tick is the original's 30 Hz frame, not a 60th, so a
/// 1 build cannot ride with a 2 (it would run a tick of the others' every frame at another length).
pub const ALPN: &[u8] = b"openmx/lobby/2";
/// How long Find listens on the LAN before it shows what it has (mDNS answers come within a second or two).
const LAN_WAIT: Duration = Duration::from_secs(3);
/// A host's listing and code go out again this often, and on a change at most every [`LIST_GAP`].
const REPUBLISH: Duration = Duration::from_secs(60);
const LIST_GAP: Duration = Duration::from_secs(10);
/// How long a code's lookup keeps asking (a code just rolled can take a while to reach the nodes asked).
const RESOLVE: Duration = Duration::from_secs(40);
/// How long the host waits for every guest's handshake after its Start.
const SHAKE_WAIT: Duration = Duration::from_secs(10);

// ---------------------------------------------------------------- settings

/// Which relay to fall back on when hole punching fails.
#[derive(Clone, Debug, PartialEq)]
pub enum Relay {
    /// n0's free public relays.
    N0,
    /// Your own `iroh-relay`.
    Own(String),
    /// None: LAN only (and direct internet when both sides can hole punch).
    Off,
}

/// Where lobbies are listed and codes written besides the LAN and the pkarr relays.
#[derive(Clone, Debug, PartialEq)]
pub enum Board {
    /// The mainline DHT.
    Mainline,
    /// A DHT of these bootstrap nodes only (a local testnet).
    Local(Vec<String>),
    /// None.
    Off,
}

/// `online.txt`: `name = …` (this player's name, on their seats; a host's lobby is listed under it), `relay = n0 |
/// own | off`, `relay_url = https://…`, `private = yes | no` (Host's last choice), `code_server = https://…` (your
/// own pkarr relay, `tools/pkarr-relay`, asked before [`codes::PKARR`]). The rest is not in the file: the terminal
/// lobby's flags and the tests set it.
#[derive(Clone, Debug, PartialEq)]
pub struct Online {
    pub name: String,
    pub relay: Relay,
    pub relay_url: String,
    pub private: bool,
    pub code_server: String,
    pub board: Board,
    /// Lobbies and addresses by mDNS.
    pub lan: bool,
    /// The pkarr relays asked for codes, in order (default: `code_server`, then [`codes::PKARR`]).
    pub code_servers: Option<Vec<String>>,
    /// No direct UDP paths: everything through the relay (testing the relay path).
    pub relay_only: bool,
}

impl Default for Online {
    fn default() -> Self {
        Online { name: "Rider".into(), relay: Relay::N0, relay_url: String::new(), private: false, code_server: String::new(), board: Board::Mainline, lan: true, code_servers: None, relay_only: false }
    }
}

impl Online {
    /// `online.txt` beside the disc image.
    pub fn path(iso: &str) -> std::path::PathBuf {
        std::path::Path::new(iso).parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(".".as_ref()).join("online.txt")
    }

    pub fn parse(text: &str) -> Online {
        let mut o = Online::default();
        let mut relay = "n0".to_string();
        for (k, v) in text.lines().filter_map(|l| l.split('#').next()?.split_once('=')) {
            let v = v.trim().to_string();
            match k.trim() {
                "name" if !v.is_empty() => o.name = v.chars().take(24).collect(),
                "relay" => relay = v,
                "relay_url" => o.relay_url = v,
                "private" => o.private = v == "yes",
                "code_server" => o.code_server = v,
                _ => {}
            }
        }
        o.relay = match relay.as_str() {
            "off" => Relay::Off,
            "own" => Relay::Own(o.relay_url.clone()),
            _ => Relay::N0,
        };
        o
    }

    pub fn text(&self) -> String {
        let relay = match self.relay {
            Relay::N0 => "n0",
            Relay::Own(_) => "own",
            Relay::Off => "off",
        };
        let code = if self.code_server.is_empty() { String::new() } else { format!("code_server = {}\n", self.code_server) };
        format!("name = {}\nrelay = {relay}\nrelay_url = {}\nprivate = {}\n{code}", self.name, self.relay_url, if self.private { "yes" } else { "no" })
    }

    /// The next relay choice for the menu: n0, Off, then own once a `relay_url` is set.
    pub fn next_relay(&mut self) {
        self.relay = match self.relay {
            Relay::N0 => Relay::Off,
            Relay::Off if !self.relay_url.is_empty() => Relay::Own(self.relay_url.clone()),
            Relay::Off | Relay::Own(_) => Relay::N0,
        };
    }

    fn servers(&self) -> Vec<String> {
        self.code_servers.clone().unwrap_or_else(|| [self.code_server.clone(), codes::PKARR.into()].into_iter().filter(|s| !s.is_empty()).collect())
    }
}

/// A typed relay URL as the endpoint will dial it, or why it can't be one.
pub fn relay_url(s: &str) -> Result<String, String> {
    let u = s.trim().parse::<RelayUrl>().map_err(|e| e.to_string())?;
    match u.scheme() {
        "http" | "https" if u.host_str().is_some_and(|h| !h.is_empty()) => Ok(u.to_string()),
        "http" | "https" => Err("no host".into()),
        s => Err(format!("{s}: not http or https")),
    }
}

/// Why a guest of another build is turned away at its Hello, for the guest to read.
pub fn version_differs(host: &str, you: &str) -> String {
    format!("a different version of the game (host {host}, you {you}). Update so you both have the same one.")
}

// ---------------------------------------------------------------- wire

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Msg {
    Hello { build: String, name: String, picks: Vec<Pick> },
    Room(Room),
    Refused(String),
    /// A guest ready to ride (or not any more), and its new picks for the seats it holds.
    Prepared(bool),
    Picks(Vec<Pick>),
    /// The host's Start: the line-up with its seed, for each guest's handshake; the handshake; the ride going.
    Start(Room),
    Shake(Handshake),
    Go(Room),
    /// Everyone back in the lobby (why: a peer dropped, or the ride ended).
    Lobby { room: Room, why: String },
    /// A line for a guest's status.
    Note(String),
    /// The host's round-trip probe (its clock in µs) and the guest's echo.
    Ping(u64),
    Pong(u64),
    /// A chat line: a guest's goes to the host with `from` and `name` empty; the host fills them in, cleans and
    /// rate-limits it, and passes it on.
    Chat { from: String, name: String, text: String },
    /// In the ride: one peer's message to every other (the host fills in `from` and relays it).
    Ride { from: String, data: Vec<u8> },
}

/// Messages out through one stream, in order, on their own task.
#[derive(Clone)]
struct Link(amp::UnboundedSender<Msg>);

impl Link {
    fn new(mut tx: SendStream) -> Link {
        let (s, mut r) = amp::unbounded_channel::<Msg>();
        n0_future::task::spawn(async move {
            while let Some(m) = r.recv().await {
                if send(&mut tx, &m).await.is_err() {
                    break;
                }
            }
        });
        Link(s)
    }

    fn send(&self, m: &Msg) {
        let _ = self.0.send(m.clone());
    }
}

/// What arrives on a connection: messages (an `Err` ends it) and datagrams.
struct Inbox {
    msgs: amp::UnboundedReceiver<Result<Msg, String>>,
    dgrams: amp::UnboundedReceiver<Vec<u8>>,
}

/// A connection with its stream and datagrams read on their own tasks (`recv` isn't cancel-safe, so it can't sit in
/// a `select!`).
fn quic(conn: &Connection, tx: SendStream, mut rx: RecvStream) -> (Link, Inbox) {
    let (mt, msgs) = amp::unbounded_channel();
    n0_future::task::spawn(async move {
        loop {
            let m = recv(&mut rx).await;
            let end = m.is_err();
            if mt.send(m).is_err() || end {
                return;
            }
        }
    });
    let (dt, dgrams) = amp::unbounded_channel();
    let c = conn.clone();
    n0_future::task::spawn(async move {
        while let Ok(d) = c.read_datagram().await {
            if dt.send(d.to_vec()).is_err() {
                return;
            }
        }
    });
    (Link::new(tx), Inbox { msgs, dgrams })
}

async fn send(s: &mut SendStream, m: &Msg) -> Result<(), String> {
    let b = serde_json::to_vec(m).unwrap();
    s.write_all(&(b.len() as u32).to_le_bytes()).await.map_err(|e| e.to_string())?;
    s.write_all(&b).await.map_err(|e| e.to_string())
}

async fn recv(r: &mut RecvStream) -> Result<Msg, String> {
    let mut n = [0; 4];
    r.read_exact(&mut n).await.map_err(|e| e.to_string())?;
    let n = u32::from_le_bytes(n) as usize;
    if n > 1 << 20 {
        return Err("message too long".into());
    }
    let mut b = vec![0; n];
    r.read_exact(&mut b).await.map_err(|e| e.to_string())?;
    serde_json::from_slice(&b).map_err(|e| e.to_string())
}

/// The most a datagram's data may be (iroh's path MTU less the peer's name).
pub const DATAGRAM_MAX: usize = 1100;

/// A datagram for (from a guest) or from (to a guest) `peer`: its name's length, the name, the data.
fn datagram(peer: &str, data: &[u8]) -> Vec<u8> {
    [&[peer.len() as u8][..], peer.as_bytes(), data].concat()
}

fn undatagram(d: &[u8]) -> Option<(String, &[u8])> {
    let (&n, rest) = d.split_first()?;
    let (peer, data) = rest.split_at_checked(n as usize)?;
    Some((String::from_utf8(peer.to_vec()).ok()?, data))
}

// ---------------------------------------------------------------- the app's handle

/// What the app asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    /// Host `room` (the host's own players already seated as [`HOST`]).
    Host(Room),
    /// List the open lobbies.
    Find,
    /// Join the busiest open lobby with room for these players (`room`'s seats), else host `room`.
    Quick(Room),
    /// Join by code (a room code, a ticket or an endpoint ID) with these players.
    Join(String, Vec<Pick>),
    /// This player's name for the lobby (before Host/Join/Quick).
    Name(String),
    /// Leave the lobby, stop hosting, or leave the ride.
    Leave,
    /// In the lobby: ready or not; this machine's players' new picks; the host's ride (track, wrecks, biplane),
    /// private or public, Start, and taking a peer out.
    Prepared(bool),
    Picks(Vec<Pick>),
    Ride { track: String, wrecks: bool, biplane: bool },
    Private(bool),
    Start,
    Kick(String),
    /// A chat line to everyone else.
    Chat(String),
    /// In the ride: a message to every other peer; a datagram to one peer (through the host); the host ending the
    /// ride (everyone back to the lobby).
    Send(Vec<u8>),
    Datagram(String, Vec<u8>),
    Lobby,
}

/// What the network tells the app.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Status(String),
    /// This machine hosts the lobby (Quickplay found none to join).
    Hosting,
    /// Hosting: the ticket (the whole address), which joins with no lookup at all.
    Code(String),
    /// Hosting: the room code (shown at once; a re-roll if it was taken comes as another).
    Short(String),
    /// This machine's peer name in the seats (a guest's endpoint ID; the host is [`HOST`]).
    Me(String),
    Found(Vec<Listing>),
    /// The lobby as it is now.
    Room(Room),
    /// The ride starts: the line-up with the seed, and which peer this machine is.
    Start(Room, String),
    /// A peer left mid-ride: everyone is back in the lobby (a `Room` follows).
    Dropped(String),
    /// Out of the lobby (refused, the host left, a code that leads nowhere): why.
    Failed(String),
    /// In the ride: another peer's message, and a datagram, with who sent it.
    Ride(String, Vec<u8>),
    Datagram(String, Vec<u8>),
    /// Another peer's chat line: who, their name, the line.
    Chat { from: String, name: String, text: String },
}

/// The network thread's handle.
pub struct Net {
    cmd: amp::UnboundedSender<Cmd>,
    events: std::sync::Mutex<mpsc::Receiver<Event>>,
}

impl Net {
    /// Starts the network on its own thread; `disc` hashes what this machine's disc holds for the handshake. In a
    /// browser (N4) on the page's event loop instead, which runs it between the page's frames.
    pub fn start(cfg: Online, disc: Arc<dyn Disc>) -> Net {
        let (cmd, cmd_rx) = amp::unbounded_channel();
        let (ev, events) = mpsc::channel();
        #[cfg(target_arch = "wasm32")]
        n0_future::task::spawn(run(cfg, cmd_rx, ev, disc));
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::Builder::new()
            .name("net".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_multi_thread().enable_all().worker_threads(2).build().unwrap();
                rt.block_on(run(cfg, cmd_rx, ev, disc));
            })
            .unwrap();
        Net { cmd, events: events.into() }
    }

    pub fn send(&self, c: Cmd) {
        let _ = self.cmd.send(c);
    }

    /// What came in since the last call.
    pub fn poll(&self) -> Vec<Event> {
        self.events.lock().unwrap().try_iter().collect()
    }

    /// The next event, waiting up to `wait` (the terminal lobby and the tests; a browser cannot wait).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn next(&self, wait: Duration) -> Option<Event> {
        self.events.lock().unwrap().recv_timeout(wait).ok()
    }
}

// ---------------------------------------------------------------- the network thread

struct Ctx {
    ep: Endpoint,
    codes: Codes,
    lan: Lan,
    lan_on: bool,
    ev: mpsc::Sender<Event>,
    name: String,
    disc: Arc<dyn Disc>,
    /// The relays are on (n0 or own).
    relays: bool,
}

impl Ctx {
    fn say(&self, e: Event) {
        let _ = self.ev.send(e);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn dht_builder(board: &Board) -> Option<n0_mainline::DhtBuilder> {
    let mut b = Dht::builder();
    match board {
        Board::Mainline => {}
        Board::Local(nodes) => {
            b.bootstrap(nodes);
        }
        Board::Off => return None,
    }
    Some(b)
}

/// The DHT for the board and the codes, as `board` says; none in a browser, which has no UDP socket (N4).
fn dht(board: &Board) -> Result<Option<Dht>, String> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = board;
        Ok(None)
    }
    #[cfg(not(target_arch = "wasm32"))]
    dht_builder(board).map(|b| b.build()).transpose().map_err(|e| format!("DHT: {e}"))
}

/// A browser's endpoint (N4): relay-only, as opentennis's is there — the page has no UDP socket, so every packet
/// goes through the relay over a WebSocket, and nothing is looked up but by its ticket or room code.
#[cfg(target_arch = "wasm32")]
pub async fn endpoint(cfg: &Online) -> Result<(Endpoint, Option<MdnsAddressLookup>), String> {
    let mode = match &cfg.relay {
        Relay::N0 => RelayMode::Default,
        Relay::Own(url) => RelayMode::Custom(url.parse::<RelayUrl>().map_err(|e| format!("relay_url {url}: {e}"))?.into()),
        Relay::Off => return Err("a browser reaches other players only through a relay: relay off leaves it none".into()),
    };
    let b = Endpoint::builder(presets::Minimal).secret_key(SecretKey::generate()).alpns(vec![ALPN.to_vec()]).relay_mode(mode);
    Ok((b.bind().await.map_err(|e| e.to_string())?, None))
}

/// This machine's endpoint, with its address lookups (the DHT's, mDNS's) as `cfg` has them.
#[cfg(not(target_arch = "wasm32"))]
pub async fn endpoint(cfg: &Online) -> Result<(Endpoint, Option<MdnsAddressLookup>), String> {
    let mode = match &cfg.relay {
        Relay::N0 => RelayMode::Default,
        Relay::Own(url) => RelayMode::Custom(url.parse::<RelayUrl>().map_err(|e| format!("relay_url {url}: {e}"))?.into()),
        Relay::Off => RelayMode::Disabled,
    };
    let mut b = Endpoint::builder(presets::Minimal).secret_key(SecretKey::generate()).alpns(vec![ALPN.to_vec()]).relay_mode(mode);
    if let Some(dht) = dht_builder(&cfg.board) {
        // the DHT record carries the direct addresses too, so a peer with relays off can still be dialled by ID
        let lookup = DhtAddressLookup::builder().dht_builder(dht).addr_filter(AddrFilter::unfiltered());
        b = b.address_lookup(lookup);
    }
    if cfg.relay_only {
        b = b.clear_ip_transports();
    }
    let ep = b.bind().await.map_err(|e| e.to_string())?;
    if !cfg.lan {
        return Ok((ep, None));
    }
    // LAN: bare IDs resolve by mDNS and hosts' listings ride on it (no internet needed); a device whose multicast
    // can't bind still goes online without LAN lobbies
    let mdns = MdnsAddressLookup::builder().service_name(board::MDNS).build(ep.id()).map_err(|e| e.to_string());
    let mdns = mdns.and_then(|m| ep.address_lookup().map_err(|e| e.to_string()).map(|l| l.add(m.clone())).map(|_| m));
    Ok((ep, mdns.inspect_err(|e| eprintln!("mDNS: {e}: no LAN lobbies")).ok()))
}

async fn run(cfg: Online, mut cmds: amp::UnboundedReceiver<Cmd>, ev: mpsc::Sender<Event>, disc: Arc<dyn Disc>) {
    let _ = ev.send(Event::Status("Connecting...".into()));
    let (ep, mdns) = match endpoint(&cfg).await {
        Ok(e) => e,
        Err(e) => {
            let _ = ev.send(Event::Failed(e));
            return;
        }
    };
    let dht = match dht(&cfg.board) {
        Ok(d) => d,
        Err(e) => {
            let _ = ev.send(Event::Failed(e));
            return;
        }
    };
    let lan_on = mdns.is_some();
    let mut cx = Ctx { ep, codes: Codes::new(dht, cfg.servers()), lan: board::watch_lan(mdns), lan_on, ev, name: cfg.name.clone(), disc, relays: cfg.relay != Relay::Off };
    cx.say(Event::Status("Online.".into()));
    let private = cfg.private;
    while let Some(c) = cmds.recv().await {
        match c {
            Cmd::Host(mut room) => {
                room.private |= private;
                host(&cx, room, &mut cmds).await
            }
            Cmd::Find => {
                cx.say(Event::Status("Looking for lobbies...".into()));
                find(&cx, |l| cx.say(Event::Found(l))).await;
            }
            Cmd::Quick(room) => {
                cx.say(Event::Status("Looking for a lobby...".into()));
                let mine: Vec<Pick> = room.seats.iter().flatten().map(|s| s.pick.clone()).collect();
                let mut tried: Vec<String> = Vec::new();
                let mut lists = Vec::new();
                find(&cx, |l| lists.push(l)).await;
                let mut joined = false;
                'look: for list in lists {
                    for l in quick_pick(&list, mine.len()) {
                        if tried.contains(&l.id) {
                            continue;
                        }
                        tried.push(l.id.clone());
                        if join(&cx, &l.id, mine.clone(), &mut cmds).await {
                            joined = true;
                            break 'look;
                        }
                    }
                }
                if !joined {
                    host(&cx, room, &mut cmds).await;
                }
            }
            Cmd::Join(code, picks) => {
                join(&cx, &code, picks, &mut cmds).await;
            }
            Cmd::Name(n) => cx.name = n,
            _ => {}
        }
    }
    cx.ep.close().await;
}

/// The open lobbies on the LAN and the DHT board, each list to `out` as an answer comes in (the LAN's after
/// [`LAN_WAIT`], the board's when it has read it), never this machine's own.
async fn find(cx: &Ctx, mut out: impl FnMut(Vec<Listing>)) {
    let me = cx.ep.id().to_string();
    let merged = |dht: &[Listing]| {
        let mut list = board::lan_list(&cx.lan);
        for l in dht {
            if !list.iter().any(|o| o.id == l.id) {
                list.push(l.clone());
            }
        }
        list.retain(|l| l.id != me);
        list
    };
    let board = async {
        match &cx.codes.dht {
            Some(d) => board::find(d).await,
            None => Vec::new(),
        }
    };
    tokio::pin!(board);
    let wait = n0_future::time::sleep(if cx.lan_on { LAN_WAIT } else { Duration::ZERO });
    tokio::pin!(wait);
    let mut dht: Option<Vec<Listing>> = None;
    let mut waited = false;
    while dht.is_none() || !waited {
        tokio::select! {
            l = &mut board, if dht.is_none() => {
                out(merged(&l));
                dht = Some(l);
            }
            _ = &mut wait, if !waited => {
                waited = true;
                out(merged(dht.as_deref().unwrap_or_default()));
            }
        }
    }
}

/// The ticket to share: the endpoint's whole address.
fn ticket(ep: &Endpoint) -> String {
    EndpointTicket::new(ep.addr()).to_string()
}

/// Waits (bounded) for the address a ticket needs: a home relay with the relays on, else a direct address.
async fn addressed(ep: &Endpoint, relays: bool) {
    if relays {
        let _ = n0_future::time::timeout(Duration::from_secs(10), ep.online()).await;
    }
    for _ in 0..50 {
        if ep.addr().ip_addrs().next().is_some() || ep.addr().relay_urls().next().is_some() {
            return;
        }
        n0_future::time::sleep(Duration::from_millis(100)).await;
    }
}

/// A joiner's side, inside the host task.
enum Guest {
    In { peer: String, name: String, picks: Vec<Pick>, build: String, link: Link, conn: Connection },
    Said(String, Msg),
    Gone(String),
    /// A datagram from the first peer for the second.
    Datagram(String, String, Vec<u8>),
}

async fn guest(gtx: amp::UnboundedSender<Guest>, peer: String, conn: Connection, link: Link, mut inbox: Inbox) {
    let Some(Ok(Msg::Hello { build, name, picks })) = inbox.msgs.recv().await else {
        conn.close(0u32.into(), b"no hello");
        return;
    };
    let _ = gtx.send(Guest::In { peer: peer.clone(), name, picks, build, link, conn });
    loop {
        tokio::select! {
            m = inbox.msgs.recv() => match m {
                Some(Ok(m)) => drop(gtx.send(Guest::Said(peer.clone(), m))),
                _ => break,
            },
            Some(d) = inbox.dgrams.recv() => if let Some((to, data)) = undatagram(&d) {
                let _ = gtx.send(Guest::Datagram(peer.clone(), to, data.to_vec()));
            },
        }
    }
    let _ = gtx.send(Guest::Gone(peer));
}

/// Refuses a guest (`why`), closing once the refusal has had time to arrive.
fn refuse(link: &Link, conn: Connection, why: String) {
    link.send(&Msg::Refused(why));
    n0_future::task::spawn(async move {
        n0_future::time::sleep(Duration::from_secs(2)).await;
        conn.close(0u32.into(), b"refused");
    });
}

/// A random seed from a fresh key's bytes (no rand dependency, as the codes).
fn seed() -> u32 {
    let b = SecretKey::generate().to_bytes();
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

struct Joined {
    peer: String,
    link: Link,
    conn: Connection,
}

fn broadcast(to: &[Joined], m: &Msg) {
    to.iter().for_each(|j| j.link.send(m));
}

/// The host's Start under way: the guests whose handshake hasn't come, the host's own, the refusals so far and
/// when it gives up waiting.
struct Shaking {
    waiting: Vec<String>,
    own: Handshake,
    refused: Vec<String>,
    until: n0_future::time::Instant,
}

async fn host(cx: &Ctx, mut room: Room, cmds: &mut amp::UnboundedReceiver<Cmd>) {
    if room.name.is_empty() {
        room.name = cx.name.clone();
    }
    room.names.insert(HOST.into(), cx.name.clone());
    cx.say(Event::Hosting);
    cx.say(Event::Me(HOST.into()));
    cx.say(Event::Room(room.clone()));
    let ep = cx.ep.clone();
    addressed(&ep, cx.relays).await;
    cx.say(Event::Code(ticket(&ep)));
    // the code shown at once, checked and published in the background (re-rolled if taken); on a LAN with nowhere
    // else to put it, it is out as soon as the mDNS listing carries it
    let first = codes::new_short();
    cx.say(Event::Short(first.clone()));
    let (rtx, mut rolled) = amp::unbounded_channel::<Result<String, String>>();
    let online = cx.codes.dht.is_some() || !cx.codes.servers.is_empty();
    if online {
        cx.say(Event::Status("Putting the code online...".into()));
        let (codes, ev, t, rtx) = (cx.codes.clone(), cx.ev.clone(), ticket(&ep), rtx.clone());
        n0_future::task::spawn(async move {
            let r = codes.roll(first, &t, |c| drop(ev.send(Event::Short(c.into())))).await;
            let _ = rtx.send(r);
        });
    } else {
        let _ = rtx.send(Ok(first));
    }
    let (gtx, mut guests) = amp::unbounded_channel::<Guest>();
    let mut joined: Vec<Joined> = Vec::new();
    let mut republish = n0_future::time::interval(REPUBLISH);
    let mut ping = n0_future::time::interval(Duration::from_secs(2));
    let t0 = Instant::now();
    let mut dirty = true;
    let mut listed: Option<Instant> = None;
    let mut rates: HashMap<String, Vec<Instant>> = HashMap::new();
    let mut shaking: Option<Shaking> = None;
    loop {
        // the listing on the LAN and the DHT board, and the code's record, after a change (at most every LIST_GAP)
        if dirty && listed.is_none_or(|t| t.elapsed() >= LIST_GAP) {
            dirty = false;
            listed = Some(Instant::now());
            let listing = Listing::of(&ep.id().to_string(), &room);
            ep.set_user_data_for_address_lookup(if room.private { None } else { board::lan_data(&listing) });
            if let Some(dht) = cx.codes.dht.clone().filter(|_| !room.private) {
                let (key, r) = (ep.secret_key().clone(), room.clone());
                n0_future::task::spawn(async move { board::announce(&dht, &key, &r).await.inspect_err(|e| eprintln!("{e}")) });
            }
        }
        let shake_end = shaking.as_ref().map_or_else(|| n0_future::time::Instant::now() + REPUBLISH, |s| s.until);
        tokio::select! {
            Some(inc) = ep.accept() => {
                let gtx = gtx.clone();
                n0_future::task::spawn(async move {
                    let Ok(accepting) = inc.accept() else { return };
                    let Ok(conn) = accepting.await else { return };
                    let peer = conn.remote_id().to_string();
                    let Ok((tx, rx)) = conn.accept_bi().await else { return };
                    let (link, inbox) = quic(&conn, tx, rx);
                    guest(gtx, peer, conn, link, inbox).await;
                });
            }
            Some(r) = rolled.recv() => match r {
                Ok(code) => {
                    room.short = code.clone();
                    cx.say(Event::Short(code));
                    cx.say(Event::Status("Waiting for riders.".into()));
                    broadcast(&joined, &Msg::Room(room.clone()));
                    cx.say(Event::Room(room.clone()));
                    (dirty, listed) = (true, None);
                    if online {
                        // republished with the listing from now on
                        let (codes, code, t) = (cx.codes.clone(), room.short.clone(), ticket(&ep));
                        let mut every = n0_future::time::interval(REPUBLISH);
                        every.tick().await;
                        let rtx = rtx.clone();
                        n0_future::task::spawn(async move {
                            loop {
                                every.tick().await;
                                if rtx.is_closed() {
                                    return;
                                }
                                if let Err(e) = codes.publish(&code, &t).await {
                                    eprintln!("room code {code}: {e}");
                                }
                            }
                        });
                    }
                }
                Err(why) => cx.say(Event::Status(format!("No room code ({why}): share the ticket instead."))),
            },
            Some(g) = guests.recv() => match g {
                Guest::In { peer, name, picks, build, link, conn } => {
                    if build != handshake::build() {
                        refuse(&link, conn, version_differs(handshake::build(), &build));
                    } else if room.riding || shaking.is_some() {
                        refuse(&link, conn, "The ride has started: try again once it's back in the lobby.".into());
                    } else if picks.is_empty() {
                        refuse(&link, conn, "No players to seat.".into());
                    } else if !room.take(&peer, &picks) {
                        refuse(&link, conn, format!("The lobby is full ({} seats open, {} wanted).", room.open(), picks.len()));
                    } else {
                        room.names.insert(peer.clone(), crate::chat::clean(&name).unwrap_or_else(|| short(&peer).into()));
                        joined.push(Joined { peer, link, conn });
                        broadcast(&joined, &Msg::Room(room.clone()));
                        cx.say(Event::Room(room.clone()));
                        dirty = true;
                    }
                }
                Guest::Said(peer, m) => match m {
                    Msg::Prepared(p) if !room.riding && shaking.is_none() => {
                        if p { room.ready.insert(peer); } else { room.ready.remove(&peer); }
                        broadcast(&joined, &Msg::Room(room.clone()));
                        cx.say(Event::Room(room.clone()));
                    }
                    Msg::Picks(p) if !room.riding && shaking.is_none() => {
                        room.repick(&peer, &p);
                        broadcast(&joined, &Msg::Room(room.clone()));
                        cx.say(Event::Room(room.clone()));
                    }
                    Msg::Shake(h) => {
                        if let Some(s) = &mut shaking && s.waiting.contains(&peer) {
                            s.waiting.retain(|p| *p != peer);
                            if let Err(why) = h.check(&s.own) {
                                s.refused.push(format!("{}: {why}", room.names.get(&peer).cloned().unwrap_or_default()));
                                if let Some(i) = joined.iter().position(|j| j.peer == peer) {
                                    let j = joined.remove(i);
                                    refuse(&j.link, j.conn, why);
                                }
                                room.free(&peer);
                            }
                        }
                    }
                    Msg::Pong(t) => {
                        let ms = (micros(t0).saturating_sub(t) / 1000) as u32;
                        if room.ping.insert(peer, ms) != Some(ms) && !room.riding {
                            broadcast(&joined, &Msg::Room(room.clone()));
                            cx.say(Event::Room(room.clone()));
                        }
                    }
                    Msg::Chat { text, .. } => chat(cx, &peer, &text, &room.names, &joined, &mut rates),
                    Msg::Ride { data, .. } if room.riding => {
                        let m = Msg::Ride { from: peer.clone(), data: data.clone() };
                        joined.iter().filter(|j| j.peer != peer).for_each(|j| j.link.send(&m));
                        cx.say(Event::Ride(peer, data));
                    }
                    _ => {}
                },
                Guest::Datagram(from, to, data) => {
                    if to == HOST {
                        cx.say(Event::Datagram(from, data));
                    } else if let Some(j) = joined.iter().find(|j| j.peer == to) {
                        let _ = j.conn.send_datagram(datagram(&from, &data).into());
                    }
                }
                Guest::Gone(peer) => {
                    let Some(i) = joined.iter().position(|j| j.peer == peer) else { continue };
                    joined.remove(i);
                    let name = room.names.get(&peer).cloned().unwrap_or_else(|| short(&peer).into());
                    room.free(&peer);
                    if let Some(s) = &mut shaking {
                        s.waiting.retain(|p| *p != peer);
                        s.refused.push(format!("{name} left"));
                    } else if room.riding {
                        room.back_to_lobby();
                        let why = format!("{name} left the ride: back to the lobby.");
                        broadcast(&joined, &Msg::Lobby { room: room.clone(), why: why.clone() });
                        cx.say(Event::Dropped(why));
                        cx.say(Event::Room(room.clone()));
                    } else {
                        broadcast(&joined, &Msg::Room(room.clone()));
                        cx.say(Event::Room(room.clone()));
                    }
                    dirty = true;
                }
            },
            c = cmds.recv() => match c {
                Some(Cmd::Leave) | None => {
                    ep.set_user_data_for_address_lookup(None);
                    for j in &joined {
                        j.conn.close(0u32.into(), b"left");
                    }
                    return;
                }
                Some(Cmd::Picks(p)) if !room.riding && shaking.is_none() => {
                    room.repick(HOST, &p);
                    broadcast(&joined, &Msg::Room(room.clone()));
                    cx.say(Event::Room(room.clone()));
                }
                Some(Cmd::Ride { track, wrecks, biplane }) if !room.riding && shaking.is_none() => {
                    (room.track, room.wrecks, room.biplane) = (track, wrecks, biplane);
                    room.changed();
                    broadcast(&joined, &Msg::Room(room.clone()));
                    cx.say(Event::Room(room.clone()));
                    dirty = true;
                }
                Some(Cmd::Private(p)) => {
                    room.private = p;
                    broadcast(&joined, &Msg::Room(room.clone()));
                    cx.say(Event::Room(room.clone()));
                    (dirty, listed) = (true, None);
                }
                Some(Cmd::Kick(peer)) if peer != HOST => {
                    if let Some(i) = joined.iter().position(|j| j.peer == peer) {
                        let j = joined.remove(i);
                        refuse(&j.link, j.conn, "The host took you out of the lobby.".into());
                        room.free(&peer);
                        broadcast(&joined, &Msg::Room(room.clone()));
                        cx.say(Event::Room(room.clone()));
                        dirty = true;
                    }
                }
                Some(Cmd::Start) if !room.riding && shaking.is_none() => {
                    if !room.all_ready() {
                        cx.say(Event::Status("Waiting for everyone to be ready.".into()));
                        continue;
                    }
                    room.seed = seed();
                    let waiting: Vec<String> = room.peers().into_iter().filter(|p| p != HOST).collect();
                    shaking = Some(Shaking { waiting, own: Handshake::of(&room, &*cx.disc), refused: Vec::new(), until: n0_future::time::Instant::now() + SHAKE_WAIT });
                    broadcast(&joined, &Msg::Start(room.clone()));
                    cx.say(Event::Status("Starting: checking the ride with everyone...".into()));
                }
                Some(Cmd::Chat(text)) => chat(cx, HOST, &text, &room.names, &joined, &mut rates),
                Some(Cmd::Send(data)) if room.riding => broadcast(&joined, &Msg::Ride { from: HOST.into(), data }),
                Some(Cmd::Datagram(to, data)) => {
                    if let Some(j) = joined.iter().find(|j| j.peer == to) {
                        let _ = j.conn.send_datagram(datagram(HOST, &data).into());
                    }
                }
                Some(Cmd::Lobby) if room.riding => {
                    room.back_to_lobby();
                    let why = "The host ended the ride: back to the lobby.".to_string();
                    broadcast(&joined, &Msg::Lobby { room: room.clone(), why });
                    cx.say(Event::Room(room.clone()));
                    dirty = true;
                }
                Some(_) => {}
            },
            _ = n0_future::time::sleep_until(shake_end), if shaking.is_some() => {
                let s = shaking.as_mut().unwrap();
                for peer in std::mem::take(&mut s.waiting) {
                    s.refused.push(format!("{}: no handshake in {} s", room.names.get(&peer).cloned().unwrap_or_default(), SHAKE_WAIT.as_secs()));
                    if let Some(i) = joined.iter().position(|j| j.peer == peer) {
                        let j = joined.remove(i);
                        refuse(&j.link, j.conn, "Your game didn't answer the host's start.".into());
                    }
                    room.free(&peer);
                }
            }
            _ = republish.tick() => dirty = true,
            _ = ping.tick() => broadcast(&joined, &Msg::Ping(micros(t0))),
        }
        // every handshake in: the ride goes, or (someone refused or gone) everyone left is back in the lobby
        if shaking.as_ref().is_some_and(|s| s.waiting.is_empty()) {
            let s = shaking.take().unwrap();
            if s.refused.is_empty() {
                room.riding = true;
                broadcast(&joined, &Msg::Go(room.clone()));
                cx.say(Event::Start(room.clone(), HOST.into()));
            } else {
                room.back_to_lobby();
                let why = format!("Not started: {}", s.refused.join("; "));
                broadcast(&joined, &Msg::Note(why.clone()));
                broadcast(&joined, &Msg::Room(room.clone()));
                cx.say(Event::Status(why));
                cx.say(Event::Room(room.clone()));
            }
            dirty = true;
        }
    }
}

fn micros(t0: Instant) -> u64 {
    t0.elapsed().as_micros() as u64
}

fn short(id: &str) -> &str {
    &id[..id.len().min(8)]
}

/// Peer `from`'s chat line ([`HOST`]: this machine's), cleaned and rate-limited (one over the limit is dropped), to
/// every other peer with the sender's lobby name, and a guest's to this app too.
fn chat(cx: &Ctx, from: &str, text: &str, names: &BTreeMap<String, String>, joined: &[Joined], rates: &mut HashMap<String, Vec<Instant>>) {
    let Some(text) = crate::chat::clean(text) else { return };
    if !crate::chat::allow(rates.entry(from.into()).or_default(), Instant::now()) {
        return;
    }
    let name = names.get(from).cloned().unwrap_or_else(|| short(from).into());
    let m = Msg::Chat { from: from.into(), name: name.clone(), text: text.clone() };
    joined.iter().filter(|j| j.peer != from).for_each(|j| j.link.send(&m));
    if from != HOST {
        cx.say(Event::Chat { from: from.into(), name, text });
    }
}

/// The address a code stands for: a room code looked up (the LAN's listings first, then the DHT and the pkarr
/// relays), a ticket, or a bare endpoint ID (its address from mDNS or the DHT).
async fn resolve(cx: &Ctx, code: &str) -> Result<EndpointAddr, String> {
    if let Ok(t) = code.parse::<EndpointTicket>() {
        return Ok(t.into());
    }
    if let Ok(id) = code.parse::<EndpointId>() {
        return Ok(id.into());
    }
    let Some(code) = codes::code_in(code) else { return Err("That isn't a room code.".into()) };
    let online = cx.codes.dht.is_some() || !cx.codes.servers.is_empty();
    if !online && !cx.lan_on {
        return Err("Room codes need the LAN or the internet, and both are off.".into());
    }
    cx.say(Event::Status(format!("Looking up {code}...")));
    let (tx, mut rx) = amp::unbounded_channel();
    if online {
        let (codes, c) = (cx.codes.clone(), code.clone());
        n0_future::task::spawn(async move {
            loop {
                let r = n0_future::time::timeout(Duration::from_secs(20), codes.lookup(&c)).await.unwrap_or_else(|_| Err("no answer in 20 s".into()));
                if tx.send(r).is_err() {
                    return;
                }
                n0_future::time::sleep(Duration::from_secs(3)).await;
            }
        });
    }
    let until = Instant::now() + if online { RESOLVE } else { LAN_WAIT * 2 };
    let mut last = Ok(None);
    loop {
        if let Some(id) = board::lan_code(&cx.lan, &code).and_then(|l| l.id.parse::<EndpointId>().ok()) {
            return Ok(id.into());
        }
        if Instant::now() >= until {
            return Err(codes::missed(&code, last));
        }
        tokio::select! {
            Some(r) = rx.recv() => {
                if let Ok(Some((t, seq))) = &r && codes::fresh(*seq) && let Ok(t) = t.parse::<EndpointTicket>() {
                    return Ok(t.into());
                }
                last = r.map(|o| o.map(|(_, seq)| seq));
            }
            _ = n0_future::time::sleep(Duration::from_millis(250)) => {}
        }
    }
}

/// Connects to the host by `code`: the connection, its link and arrivals; None (the failure said) if it didn't.
async fn connect(cx: &Ctx, code: &str) -> Option<(Connection, Link, Inbox)> {
    let addr = match resolve(cx, code.trim()).await {
        Ok(a) => a,
        Err(why) => {
            cx.say(Event::Failed(why));
            return None;
        }
    };
    if !cx.relays && addr.ip_addrs().next().is_none() && addr.relay_urls().next().is_some() {
        cx.say(Event::Failed("That lobby can only be reached through a relay: set Relay (Online) to n0 to join it.".into()));
        return None;
    }
    cx.say(Event::Status(format!("Joining {}...", codes::code_in(code).unwrap_or_else(|| short(&addr.id.to_string()).into()))));
    let conn = match n0_future::time::timeout(Duration::from_secs(30), cx.ep.connect(addr, ALPN)).await {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => {
            cx.say(Event::Failed(format!("Couldn't reach the host: {e}")));
            return None;
        }
        Err(_) => {
            cx.say(Event::Failed("Couldn't reach the host (timed out).".into()));
            return None;
        }
    };
    let Ok((tx, rx)) = conn.open_bi().await else {
        cx.say(Event::Failed("The host closed the connection.".into()));
        return None;
    };
    let (link, inbox) = quic(&conn, tx, rx);
    Some((conn, link, inbox))
}

/// Joins by code; true once seated (the lobby then runs until this machine leaves or the host goes).
async fn join(cx: &Ctx, code: &str, picks: Vec<Pick>, cmds: &mut amp::UnboundedReceiver<Cmd>) -> bool {
    if cx.relays {
        let _ = n0_future::time::timeout(Duration::from_secs(10), cx.ep.online()).await;
    }
    let Some((conn, link, inbox)) = connect(cx, code).await else { return false };
    let me = cx.ep.id().to_string();
    link.send(&Msg::Hello { build: handshake::build().into(), name: cx.name.clone(), picks });
    cx.say(Event::Me(me.clone()));
    let Inbox { mut msgs, mut dgrams } = inbox;
    let mut seated = false;
    let mut riding = false;
    let hello = n0_future::time::sleep(Duration::from_secs(20));
    tokio::pin!(hello);
    cx.say(Event::Status("Connected: waiting for the host's lobby...".into()));
    loop {
        tokio::select! {
            _ = &mut hello, if !seated => {
                cx.say(Event::Failed("The host didn't answer (no lobby after 20 s).".into()));
                conn.close(0u32.into(), b"left");
                return false;
            }
            m = msgs.recv() => match m.unwrap_or_else(|| Err("closed".into())) {
                Ok(Msg::Room(r)) => {
                    if !seated {
                        cx.say(Event::Status("Joined: pick your rider and bike, then Ready.".into()));
                    }
                    seated = true;
                    cx.say(Event::Room(r));
                }
                Ok(Msg::Start(r)) => {
                    link.send(&Msg::Shake(Handshake::of(&r, &*cx.disc)));
                    cx.say(Event::Status("Starting: checking the ride with the host...".into()));
                }
                Ok(Msg::Go(r)) => {
                    riding = true;
                    cx.say(Event::Start(r, me.clone()));
                }
                Ok(Msg::Lobby { room, why }) => {
                    riding = false;
                    cx.say(Event::Dropped(why));
                    cx.say(Event::Room(room));
                }
                Ok(Msg::Note(s)) => cx.say(Event::Status(s)),
                Ok(Msg::Ping(t)) => link.send(&Msg::Pong(t)),
                Ok(Msg::Chat { from, name, text }) if seated => cx.say(Event::Chat { from, name, text }),
                Ok(Msg::Ride { from, data }) if riding => cx.say(Event::Ride(from, data)),
                Ok(Msg::Refused(why)) => {
                    cx.say(Event::Failed(format!("Refused: {why}")));
                    conn.close(0u32.into(), b"refused");
                    return false;
                }
                Ok(_) => {}
                Err(_) => {
                    cx.say(Event::Failed(if seated { "The host left." } else { "The host closed the connection." }.into()));
                    return seated;
                }
            },
            Some(d) = dgrams.recv() => if riding && let Some((from, data)) = undatagram(&d) {
                cx.say(Event::Datagram(from, data.to_vec()));
            },
            c = cmds.recv() => match c {
                Some(Cmd::Prepared(p)) => link.send(&Msg::Prepared(p)),
                Some(Cmd::Picks(p)) => link.send(&Msg::Picks(p)),
                Some(Cmd::Chat(text)) => link.send(&Msg::Chat { from: String::new(), name: String::new(), text }),
                Some(Cmd::Send(data)) if riding => link.send(&Msg::Ride { from: String::new(), data }),
                Some(Cmd::Datagram(to, data)) => drop(conn.send_datagram(datagram(&to, &data).into())),
                Some(Cmd::Leave) | None => {
                    // a moment for what was queued to go out first
                    n0_future::time::sleep(Duration::from_millis(100)).await;
                    conn.close(0u32.into(), b"left");
                    return true;
                }
                Some(_) => {}
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn online_txt_round_trips() {
        let o = Online { name: "Ricky".into(), relay: Relay::Own("https://r.example/".into()), relay_url: "https://r.example/".into(), private: true, code_server: "https://pkarr.example".into(), ..Online::default() };
        assert_eq!(Online::parse(&o.text()), o);
        assert_eq!(Online::parse("name = \nrelay = off # LAN"), Online { relay: Relay::Off, ..Online::default() });
        assert_eq!(o.servers(), ["https://pkarr.example", codes::PKARR]);
        let mut c = Online::default();
        c.next_relay();
        assert_eq!(c.relay, Relay::Off);
        c.next_relay();
        assert_eq!(c.relay, Relay::N0, "no relay_url: own is skipped");
        assert!(relay_url("ftp://x").is_err() && relay_url("http://127.0.0.1:3340").is_ok());
    }

    #[test]
    fn datagrams_carry_their_peer() {
        let d = datagram("abc", &[1, 2, 3]);
        assert_eq!(undatagram(&d), Some(("abc".into(), &[1u8, 2, 3][..])));
        assert_eq!(undatagram(&[9, 1]), None);
    }
}
