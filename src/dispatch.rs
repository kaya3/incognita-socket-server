use std::collections::HashMap;
use async_std::prelude::*;
use async_std::io;
use async_std::net::{TcpListener, TcpStream, SocketAddr};
use futures::{SinkExt, StreamExt};
use futures::channel::mpsc;

use crate::err;
use crate::models::UserID;
use crate::request;
use crate::response;
use crate::server::Server;

struct UserIdent {
    id: UserID,
    addr: SocketAddr,
}

impl std::fmt::Display for UserIdent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let UserIdent {id, addr} = self;
        write!(f, "User #{id} @ {addr}")
    }
}

pub(crate) type Sender<T> = mpsc::UnboundedSender<T>;
pub(crate) type Receiver<T> = mpsc::UnboundedReceiver<T>;

pub(crate) async fn start_server(server: Server, host: &str, port: u16) -> err::Result {
    let server_addr = format!("{host}:{port}");
    let listener = TcpListener::bind(&server_addr).await?;
    println!("Listening on {server_addr}");
    
    let dispatcher = Dispatcher::new(server);
    let mut dispatcher_send = dispatcher.out.clone();
    let dispatcher_task = err::spawn_logged_task(dispatcher.run());
    
    println!("Waiting for connections...");
    
    let mut incoming = listener.incoming();
    while let Some(conn) = incoming.next().await {
        let Ok((conn, addr)) = conn
            .and_then(|s| {
                s.peer_addr().map(|a| (s, a))
            })
            .map_err(|e| println!("Failed connection: {e}"))
            else { continue; };
        
        dispatcher_send.send(Event::Connected(conn, addr)).await?;
    }
    
    drop(dispatcher_send);
    dispatcher_task.await;
    Ok(())
}

pub(crate) enum Event {
    Connected(TcpStream, SocketAddr),
    Request(UserID, request::Request),
    Disconnected(UserID, Receiver<response::Message>),
}

struct Dispatcher {
    server: Server,
    conns: HashMap<UserID, Sender<response::Message>>,
    in_: Receiver<Event>,
    out: Sender<Event>,
}

impl Dispatcher {
    fn new(server: Server) -> Dispatcher {
        let (out, in_) = mpsc::unbounded();
        Dispatcher {
            server,
            conns: HashMap::new(),
            in_,
            out,
        }
    }
    
    fn add_user(&mut self) -> Option<(UserID, Receiver<response::Message>)> {
        let user_id = self.server.add_user()?;
        let (sender, receiver) = mpsc::unbounded();
        self.conns.insert(user_id, sender);
        Some((user_id, receiver))
    }
    
    fn remove_user(&mut self, user_id: UserID) -> err::Result {
        self.server.remove_user(user_id)?;
        self.conns.remove(&user_id);
        Ok(())
    }
    
    async fn send(&mut self, user_id: UserID, msg: response::Message) {
        if let Some(out) = self.conns.get_mut(&user_id) {
            if let Err(e) = out.send(msg).await {
                println!("Error dispatching message to User #{user_id}: {e}");
            }
        }
    }
    
    async fn dispatch_response(&mut self, user_id: UserID, response: response::Response) {
        if let Some(msg) = response.returns {
            self.send(user_id, msg).await;
        }
        for (other_id, msg) in response.sends.into_iter() {
            self.send(other_id, msg).await;
        }
    }
    
    async fn run(mut self) -> err::Result {
        while let Some(event) = self.in_.next().await {
            match event {
                Event::Connected(conn, addr) => {
                    if let Some((id, mut user_messages)) = self.add_user() {
                        let user = UserHandle {
                            ident: UserIdent {id, addr},
                            conn,
                            dispatcher: self.out.clone(),
                        };
                        let mut disconnect_handle = self.out.clone();
                        err::spawn_logged_task(async move {
                            let r = user.run(&mut user_messages).await;
                            disconnect_handle.send(Event::Disconnected(id, user_messages)).await?;
                            r
                        });
                    } else {
                        println!("Failed connection from {addr}: connection limit reached");
                        let mut writer = io::BufWriter::new(&conn);
                        write_message(&mut writer, response::SERVER_FULL).await
                            .ok();
                    }
                },
                Event::Request(user_id, request) => {
                    let response = self.server.handle_request(user_id, request);
                    self.dispatch_response(user_id, response).await;
                },
                Event::Disconnected(user_id, _) => {
                    self.remove_user(user_id)?;
                },
            }
        }
        Ok(())
    }
}

struct UserHandle {
    ident: UserIdent,
    conn: TcpStream,
    dispatcher: Sender<Event>,
}

impl UserHandle {
    pub(crate) async fn run(mut self, messages: &mut Receiver<response::Message>) -> err::Result {
        let ident = self.ident;
        println!("Connected {ident}");
        
        let mut messages = messages.fuse();
        let mut in_ = io::BufReader::new(&self.conn).lines().fuse();
        let mut out = io::BufWriter::new(&self.conn);
        
        write_message(&mut out, response::Message::Welcome(ident.id)).await?;
        
        loop {
            futures::select! {
                line = in_.next() => {
                    let Ok(Some(line)) = line.transpose()
                        .map_err(|e| println!("Read error from {ident}: {e}"))
                        else { break; };
                    
                    println!("Received from {ident}: {line}");
                    match request::parse(&line) {
                        Some(request) => if request.is_quit() {
                            break;
                        } else {
                            self.dispatcher.send(Event::Request(ident.id, request)).await?;
                        },
                        None => {
                            write_message(&mut out, response::INVALID_REQUEST).await?;
                        },
                    }
                },
                msg = messages.next() => {
                    let Some(msg) = msg else { break; };
                    println!("Sending to {ident}: {msg}");
                    write_message(&mut out, msg).await?;
                },
            }
        }
        
        println!("Disconnected {ident}");
        Ok(())
    }
}

async fn write_message(writer: &mut io::BufWriter<&TcpStream>, msg: response::Message) -> err::Result {
    let msg = format!("{msg}\n");
    writer.write_all(msg.as_bytes()).await?;
    writer.flush().await?;
    Ok(())
}
