//! N3: a host and a joiner find each other by room code over the LAN (mDNS, no relay, no internet) and over a
//! local relay (an in-process `iroh-relay`, every packet through it, the code through a local pkarr relay), and a
//! joiner whose disc holds a different bike set is refused at the host's Start. Also: a ride starts on both with the
//! host's seed, chat goes through the host, and a peer dropping mid-ride sends the rest back to the lobby.
//!
//! Nothing here reaches the internet: the DHT board is off, the code servers are the test's own.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use iroh::{PublicKey, Signature};
use openmx_net::{Board, Cmd, Disc, Event, HOST, Net, Online, Pick, Relay, Room};

/// A disc whose bike sets hash as given (unknown names: not on it).
struct Bikes(HashMap<&'static str, u64>);

impl Disc for Bikes {
    fn track(&self, name: &str) -> Option<u64> {
        Some(openmx_net::handshake::fnv(name.as_bytes()))
    }
    fn bike(&self, name: &str) -> Option<u64> {
        self.0.get(name).copied()
    }
}

fn disc(race125p: u64) -> Arc<dyn Disc> {
    Arc::new(Bikes(HashMap::from([("race125p", race125p), ("pro250a", 7)])))
}

/// The next event `pick` takes within `secs`, else a panic listing what came instead.
fn wait<T>(net: &Net, who: &str, secs: u64, mut pick: impl FnMut(&Event) -> Option<T>) -> T {
    let until = Instant::now() + Duration::from_secs(secs);
    let mut seen = Vec::new();
    while let Some(left) = until.checked_duration_since(Instant::now()) {
        if let Some(e) = net.next(left) {
            if let Some(t) = pick(&e) {
                return t;
            }
            seen.push(e);
        }
    }
    panic!("{who}: nothing wanted in {secs} s; saw {seen:#?}");
}

fn room_with(net: &Net, who: &str, mut ok: impl FnMut(&Room) -> bool) -> Room {
    wait(net, who, 30, |e| match e {
        Event::Room(r) if ok(r) => Some(r.clone()),
        _ => None,
    })
}

/// Hosts a two-seat free06 lobby; its room code once it is out (the code the host's lobby carries).
fn host(net: &Net) -> String {
    let mut r = Room::new("", "free06", 2);
    r.take(HOST, &[Pick::new("thq1", "race125p")]);
    net.send(Cmd::Host(r));
    room_with(net, "host", |r| !r.short.is_empty()).short
}

/// A guest joins `code` with race125p and readies; the host starts.
fn join_and_start(h: &Net, g: &Net, code: &str) {
    g.send(Cmd::Join(code.into(), vec![Pick::new("thq2", "race125p")]));
    let r = room_with(g, "guest", |r| r.taken() == 2);
    assert_eq!(r.short, code, "the guest's lobby shows the same code to pass on");
    room_with(h, "host", |r| r.taken() == 2);
    g.send(Cmd::Prepared(true));
    room_with(h, "host", |r| r.all_ready());
    h.send(Cmd::Start);
}

#[test]
fn a_host_and_a_joiner_find_each_other_by_room_code_on_the_lan() {
    let lan = |name: &str| Online { name: name.into(), relay: Relay::Off, board: Board::Off, lan: true, code_servers: Some(vec![]), ..Online::default() };
    let h = Net::start(lan("Hosty"), disc(1));
    let g = Net::start(lan("Guesty"), disc(1));
    let code = host(&h);
    join_and_start(&h, &g, &code);
    let (hr, gr) = (
        wait(&h, "host", 20, |e| if let Event::Start(r, me) = e { Some((r.clone(), me.clone())) } else { None }),
        wait(&g, "guest", 20, |e| if let Event::Start(r, me) = e { Some((r.clone(), me.clone())) } else { None }),
    );
    assert_eq!(hr.0.seed, gr.0.seed, "one seed for the ride");
    assert_eq!((hr.1.as_str(), gr.0.names.get(HOST).map(String::as_str)), (HOST, Some("Hosty")));
    assert!(gr.0.seats[1].as_ref().is_some_and(|s| s.peer == gr.1), "the guest is in seat 2");
    // the ride's traffic both ways through the lobby's connection
    g.send(Cmd::Send(b"inputs".to_vec()));
    assert_eq!(wait(&h, "host", 10, |e| if let Event::Ride(f, d) = e { Some((f.clone(), d.clone())) } else { None }), (gr.1.clone(), b"inputs".to_vec()));
    // the guest drops mid-ride: the host is back in the lobby
    g.send(Cmd::Leave);
    wait(&h, "host", 20, |e| matches!(e, Event::Dropped(_)).then_some(()));
    let back = room_with(&h, "host", |r| !r.riding);
    assert_eq!((back.taken(), back.seed), (1, 0));
}

/// A pkarr relay's GET and PUT of signed records by z-base-32 key, as `tools/pkarr-relay` runs it (the signature
/// checked, an older seq refused). Its URL.
fn pkarr_relay() -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", l.local_addr().unwrap());
    let store: Arc<Mutex<HashMap<String, Vec<u8>>>> = Arc::default();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let store = store.clone();
            std::thread::spawn(move || {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut line = String::new();
                r.read_line(&mut line).unwrap();
                let mut parts = line.split_whitespace();
                let (method, path) = (parts.next().unwrap_or_default().to_string(), parts.next().unwrap_or_default());
                let key = path.trim_start_matches('/').split('?').next().unwrap_or_default().to_string();
                let mut len = 0;
                loop {
                    let mut h = String::new();
                    if r.read_line(&mut h).unwrap_or(0) == 0 || h.trim().is_empty() {
                        break;
                    }
                    if let Some((k, v)) = h.split_once(':')
                        && k.eq_ignore_ascii_case("content-length")
                    {
                        len = v.trim().parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0; len];
                r.read_exact(&mut body).unwrap();
                let (status, out) = match method.as_str() {
                    "GET" => match store.lock().unwrap().get(&key) {
                        Some(b) => ("200 OK", b.clone()),
                        None => ("404 Not Found", Vec::new()),
                    },
                    "PUT" => {
                        let signed = || -> Option<()> {
                            let pk = PublicKey::from_z32(&key).ok()?;
                            let (sig, rest) = body.split_at_checked(64)?;
                            let (seq, v) = rest.split_at_checked(8)?;
                            let seq = i64::from_be_bytes(seq.try_into().ok()?);
                            let msg = [format!("3:seqi{seq}e1:v{}:", v.len()).into_bytes(), v.to_vec()].concat();
                            pk.verify(&msg, &Signature::from_bytes(sig.try_into().ok()?)).ok()
                        };
                        let mut st = store.lock().unwrap();
                        let older = st.get(&key).is_some_and(|b| b[64..72] > body[64..72]);
                        if signed().is_none() {
                            ("400 Bad Request", Vec::new())
                        } else if older {
                            ("409 Conflict", Vec::new())
                        } else {
                            st.insert(key, body);
                            ("204 No Content", Vec::new())
                        }
                    }
                    _ => ("405 Method Not Allowed", Vec::new()),
                };
                let mut s = s;
                let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", out.len());
                let _ = s.write_all(&out);
            });
        }
    });
    url
}

/// iroh's relay server on plain HTTP, as `iroh-relay --dev` runs it: its URL, and the runtime it runs on.
fn local_relay() -> (String, tokio::runtime::Runtime) {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().worker_threads(1).build().unwrap();
    let url = rt.block_on(async {
        let mut cfg = iroh_relay::server::ServerConfig::default();
        cfg.relay = Some(iroh_relay::server::RelayConfig::new(([127, 0, 0, 1], 0)));
        let server = iroh_relay::server::Server::spawn(cfg).await.expect("the local relay");
        let url = format!("http://{}", server.http_addr().unwrap());
        // the server runs as long as the runtime does
        Box::leak(Box::new(server));
        url
    });
    (url, rt)
}

/// Relay-only endpoints (no direct paths at all), no LAN, the code through the local pkarr relay.
fn through_relay(name: &str, relay: &str, pkarr: &str) -> Online {
    Online { name: name.into(), relay: Relay::Own(relay.into()), relay_url: relay.into(), board: Board::Off, lan: false, code_servers: Some(vec![pkarr.into()]), relay_only: true, ..Online::default() }
}

#[test]
fn a_host_and_a_joiner_find_each_other_by_room_code_through_a_local_relay() {
    let (relay, _rt) = local_relay();
    let pkarr = pkarr_relay();
    let h = Net::start(through_relay("Hosty", &relay, &pkarr), disc(1));
    let g = Net::start(through_relay("Guesty", &relay, &pkarr), disc(1));
    let code = host(&h);
    g.send(Cmd::Join(code.clone(), vec![Pick::new("thq2", "pro250a")]));
    let r = room_with(&g, "guest", |r| r.taken() == 2);
    assert_eq!(r.seats[1].as_ref().unwrap().pick.bike, "pro250a");
    // chat goes through the host with the sender's lobby name
    g.send(Cmd::Chat("  hi\u{202e} ".into()));
    assert_eq!(wait(&h, "host", 10, |e| if let Event::Chat { name, text, .. } = e { Some((name.clone(), text.clone())) } else { None }), ("Guesty".into(), "hi".into()));
    h.send(Cmd::Chat("go".into()));
    assert_eq!(wait(&g, "guest", 10, |e| if let Event::Chat { name, text, .. } = e { Some((name.clone(), text.clone())) } else { None }), ("Hosty".into(), "go".into()));
    g.send(Cmd::Prepared(true));
    room_with(&h, "host", |r| r.all_ready());
    h.send(Cmd::Start);
    wait(&h, "host", 20, |e| matches!(e, Event::Start(..)).then_some(()));
    wait(&g, "guest", 20, |e| matches!(e, Event::Start(..)).then_some(()));
}

#[test]
fn a_joiner_whose_bike_set_differs_is_refused() {
    let (relay, _rt) = local_relay();
    let pkarr = pkarr_relay();
    let h = Net::start(through_relay("Hosty", &relay, &pkarr), disc(1));
    // the guest's race125p is not the host's (another disc, or an edited .phy)
    let g = Net::start(through_relay("Guesty", &relay, &pkarr), disc(2));
    let code = host(&h);
    join_and_start(&h, &g, &code);
    let why = wait(&g, "guest", 20, |e| if let Event::Failed(w) = e { Some(w.clone()) } else { None });
    assert!(why.contains("bike set race125p isn't the same"), "{why}");
    let note = wait(&h, "host", 20, |e| if let Event::Status(s) = e { s.starts_with("Not started").then(|| s.clone()) } else { None });
    assert!(note.contains("Guesty"), "{note}");
    let back = room_with(&h, "host", |r| !r.riding && r.taken() == 1);
    assert_eq!(back.seed, 0, "no ride");
}
