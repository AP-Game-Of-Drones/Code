#![allow(unused)]

use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use game_of_drones::GameOfDrones;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;
use std::{fs, thread};
use wg_2024::config::Config;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::drone::Drone;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::*;

///Beware thy who enter: Highly repetitive code 

struct SimulationController {
    droness: HashMap<NodeId, Vec<NodeId>>,
    drones: HashMap<NodeId, Sender<DroneCommand>>,
    node_event_recv: Receiver<DroneEvent>,
}

impl SimulationController {
    fn crash_all(&mut self, id: u8) {
        if let Some(res) = self.drones.get_key_value(&id) {
            for s in self.droness.get(&id) {
                for ss in s {
                    self.drones
                        .get(ss)
                        .unwrap()
                        .send(DroneCommand::RemoveSender(id));
                }
            }
            self.drones.get(&id).unwrap().send(DroneCommand::Crash);
        } else {
            println!("Drone does not exist");
        }
    }
}

fn parse_config(file: &str) -> Config {
    let file_str = fs::read_to_string(file).unwrap();
    toml::from_str(&file_str).unwrap()
}

fn main() {

    //flood_main();

    //modify the routing for different behaviors;

    //nack_receiving_main();
    //msg_main();
    // crash();
}

#[derive(Clone)]
pub struct Client {
    pub id: NodeId,
    pub controller_send: Sender<DroneEvent>,
    pub controller_recv: Receiver<DroneCommand>,
    pub packet_recv: Receiver<Packet>,
    pub packet_send: HashMap<NodeId, Sender<Packet>>,
    pub flood_id: u64,
}

#[derive(Clone)]
pub struct Server {
    pub id: NodeId,
    pub controller_send: Sender<DroneEvent>,
    pub controller_recv: Receiver<DroneCommand>,
    pub packet_recv: Receiver<Packet>,
    pub packet_send: HashMap<NodeId, Sender<Packet>>,
    pub flood_ids: HashSet<u64>,
}

impl Client {
    fn run(&mut self) {
            loop {
                select_biased! {
                    recv(self.controller_recv) -> command_res => {
                        if let Ok(command) = command_res {
                            match command {
                                DroneCommand::Crash=>{break;},
                                _=>{}
                            }
                        }
                    },
                    recv(self.packet_recv) -> packet_res => {
                        thread::sleep(Duration::from_secs(1));
                        if let Ok(packet) = packet_res {
                            match packet.pack_type {
                                PacketType::FloodResponse(mut f)=>{
                                    if f.path_trace.clone()[0].0!=self.id{
                                        f.path_trace.reverse();
                                        f.path_trace.push((self.id,NodeType::Client));
                                        f.path_trace.reverse();
                                        println!("Yeee {:?}",f.path_trace);
                                    } else {
                                        println!("Fuck");
                                        println!("Yee {:?}",f.path_trace);
                                    }
                                },
                                PacketType::Nack(n)=>{
                                    println!("{:?}",n);
                                },
                                PacketType::Ack(_)=>{println!("Ack")},
                                PacketType::FloodRequest(f)=>{println!("FR {}",f)},
                                _=>{}
                                }
                        }
                    },
                }
            }
        }
}

impl Server {
    fn run(&mut self) {
        loop {
            select_biased! {
                recv(self.controller_recv) -> command_res => {
                    if let Ok(command) = command_res {
                        match command {
                            DroneCommand::Crash=>{break;},
                            _=>{}
                        }
                    }
                },
                recv(self.packet_recv) -> packet_res => {
                    if let Ok(mut packet) = packet_res {
                        match packet.clone().pack_type {
                            PacketType::FloodResponse(mut f)=>{
                                // println!("Yee {:?}",f.path_trace.clone());
                                packet.routing_header.increase_hop_index();
                                self.packet_send.get(&packet.routing_header.current_hop().unwrap()).unwrap().send(packet.clone());
                            },
                            PacketType::FloodRequest(mut f)=>{
                                f.path_trace.push((self.id,NodeType::Server));
                                if let Some(_id) = self.flood_ids.get(&f.flood_id){
                                    let mut packet_t: Packet = Packet {
                                        pack_type: PacketType::FloodResponse(FloodResponse{
                                            flood_id: f.flood_id,
                                            path_trace: f.path_trace.clone(),
                                        }),
                                        routing_header: SourceRoutingHeader{
                                            hop_index: 1,
                                            hops: f.path_trace.clone().into_iter().map(|f| f.0).collect::<Vec<u8>>(),
                                        },
                                        session_id: packet.session_id,
                                    };
                                    packet_t.routing_header.hops.reverse();
                                    if let Some(destination) = packet_t.routing_header.hops.last() {
                                        if *destination != f.initiator_id {
                                            packet_t.routing_header.hops.push(f.initiator_id);
                                        }
                                    }
                                    let next_hop = packet_t.clone().routing_header.hops[packet_t.clone().routing_header.hop_index];
                                    self.packet_send.get(&next_hop).unwrap().send(packet_t.clone());
                                }
                                else {
                                    // f.path_trace.push((self.id,NodeType::Server));
                                    self.flood_ids.insert(f.flood_id);
                                    if self.packet_send.clone().len() > 1 {
                                        let prev_hop = f.path_trace[f.path_trace.len()-2].0;
                                        for send_to in self.packet_send.clone().into_iter(){
                                            let sub ;
                                            if f.path_trace[0].0 != f.initiator_id
                                                && f.path_trace.clone().len() < 2
                                            {
                                                sub = 1;
                                            } else {
                                                sub = 2;
                                            }
                                            if send_to.0
                                            != f.path_trace.clone()[f.path_trace.clone().len() - sub].0{
                                                let packet_r = Packet {
                                                    pack_type: PacketType::FloodRequest(FloodRequest{
                                                        initiator_id: f.initiator_id,
                                                        flood_id: f.flood_id,
                                                        path_trace: f.path_trace.clone(),
                                                    }),
                                                    routing_header: SourceRoutingHeader {
                                                        hop_index: 1,
                                                        hops: [].to_vec(),
                                                    },
                                                    session_id:packet.clone().session_id,
                                                };

                                                send_to.1.send(packet_r.clone()).ok();
                                            }
                                        }
                                    }
                                    else {
                                        let mut packet_t: Packet = Packet {
                                        pack_type: PacketType::FloodResponse(FloodResponse{
                                            flood_id: f.flood_id,
                                            path_trace: f.path_trace.clone(),
                                        }),
                                        routing_header: SourceRoutingHeader{
                                            hop_index: 1,
                                            hops: f.path_trace.clone().into_iter().map(|f| f.0).collect::<Vec<u8>>(),
                                        },
                                        session_id: packet.session_id,
                                        };
                                        packet_t.routing_header.hops.reverse();
                                        if let Some(destination) = packet_t.routing_header.hops.last() {
                                            if *destination != f.initiator_id {
                                                packet_t.routing_header.hops.push(f.initiator_id);
                                            }
                                        }
                                        let next_hop = packet_t.clone().routing_header.hops[packet_t.clone().routing_header.hop_index];

                                        println!("{:?} {} {}",packet_t.routing_header.clone(),self.id, next_hop);

                                        self.packet_send.get(&next_hop).unwrap().send(packet_t.clone());
                                    }
                                }
                            },
                            PacketType::MsgFragment(m)=>{
                                println!("{:?}",m);
                                break;
                            },
                            _=>{}
                        }
                    }
                },
            }
        }
    }
}


fn flood_main() {

    let config = parse_config("config.toml");
    println!("{:?}", config.drone.clone());
    println!("{:?}", config.client.clone());

    let mut dd = HashMap::new();
    let mut controller_drones = HashMap::new();
    let (node_event_send, node_event_recv) = unbounded();

    let mut packet_channels = HashMap::new();
    for drone in config.drone.iter() {
        packet_channels.insert(drone.id, unbounded());
    }
    for client in config.client.iter() {
        packet_channels.insert(client.id, unbounded());
    }
    for server in config.server.iter() {
        packet_channels.insert(server.id, unbounded());
    }

    let mut handles = Vec::new();
    let mut handles_c = Vec::new();
    for drone in config.drone.into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(drone.id, controller_drone_send);
        let node_event_send = node_event_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_node_ids
            .clone()
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();
        dd.insert(drone.id, drone.connected_node_ids.clone());
        handles.push(thread::spawn(move || {
            let mut drone = GameOfDrones::new(
                drone.id,
                node_event_send,
                controller_drone_recv,
                packet_recv,
                packet_send,
                drone.pdr,
            );
            println!("{}  {:?}", drone.id, drone.packet_send.clone());

            drone.run();
        }));
    }

    // let mut clients = Vec::new();
    for drone in config.client.into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(drone.id, controller_drone_send);
        let node_event_send = node_event_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_drone_ids
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();

        let mut drone = Client {
            id: drone.id,
            controller_recv: controller_drone_recv,
            controller_send: node_event_send,
            packet_recv,
            packet_send,
            flood_id: 1,
        };
        handles_c.push(thread::spawn(|| drone));
    }

    // let mut servers = Vec::new();

    for drone in config.server.into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(drone.id, controller_drone_send);
        let node_event_send = node_event_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_drone_ids
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();

        handles.push(thread::spawn(move || {
            let mut drone = Server {
                id: drone.id,
                controller_recv: controller_drone_recv,
                controller_send: node_event_send,
                packet_recv,
                packet_send,
                flood_ids: HashSet::new(),
            };
            drone.run();
        }));
    }

    let mut controller = SimulationController {
        droness: dd,
        drones: controller_drones,
        node_event_recv,
    };


    match handles_c.pop() {
        Some(c) => {
            match c.join() {
                Ok(mut cc) => {
                    if cc.id == 5 {
                        cc.packet_send.get(&1).unwrap().send(Packet {
                            pack_type: PacketType::FloodRequest(FloodRequest {
                                flood_id: 1,
                                initiator_id: cc.id,
                                path_trace: [].to_vec(),
                            }),
                            routing_header: SourceRoutingHeader {
                                hop_index: 1,
                                hops: Vec::new(),
                            },
                            session_id: 1,
                        });
                        // println!("Waiting to receive");
                        // thread::sleep(Duration::from_secs(3));
                        println!("Trying to receive");
                        cc.run();
                    } else {
                    }
                }
                _ => {}
            }
        }
        None => {}
    }

    while let Some(handle) = handles.pop() {
        handle.join().unwrap();
    }

}


fn msg_main(){
    let config = parse_config("config.toml");
    println!("{:?}", config.drone.clone());
    println!("{:?}", config.client.clone());

    let mut dd = HashMap::new();
    let mut controller_drones = HashMap::new();
    let (node_event_send, node_event_recv) = unbounded();

    let mut packet_channels = HashMap::new();
    for drone in config.drone.iter() {
        packet_channels.insert(drone.id, unbounded());
    }
    for client in config.client.iter() {
        packet_channels.insert(client.id, unbounded());
    }
    for server in config.server.iter() {
        packet_channels.insert(server.id, unbounded());
    }

    let mut handles = Vec::new();
    let mut handles_c = Vec::new();
    for drone in config.drone.into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(drone.id, controller_drone_send);
        let node_event_send = node_event_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_node_ids
            .clone()
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();
        dd.insert(drone.id, drone.connected_node_ids.clone());
        handles.push(thread::spawn(move || {
            let mut drone = GameOfDrones::new(
                drone.id,
                node_event_send,
                controller_drone_recv,
                packet_recv,
                packet_send,
                drone.pdr,
            );
            println!("{}  {:?}", drone.id, drone.packet_send.clone());

            drone.run();
        }));
    }

    // let mut clients = Vec::new();
    for drone in config.client.into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(drone.id, controller_drone_send);
        let node_event_send = node_event_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_drone_ids
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();

        let mut drone = Client {
            id: drone.id,
            controller_recv: controller_drone_recv,
            controller_send: node_event_send,
            packet_recv,
            packet_send,
            flood_id: 1,
        };
        handles_c.push(thread::spawn(|| drone));
    }

    // let mut servers = Vec::new();

    for drone in config.server.into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(drone.id, controller_drone_send);
        let node_event_send = node_event_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_drone_ids
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();

        handles.push(thread::spawn(move || {
            let mut drone = Server {
                id: drone.id,
                controller_recv: controller_drone_recv,
                controller_send: node_event_send,
                packet_recv,
                packet_send,
                flood_ids: HashSet::new(),
            };
            drone.run();
        }));
    }

    let mut controller = SimulationController {
        droness: dd,
        drones: controller_drones,
        node_event_recv,
    };

    let res = Packet {
        pack_type: PacketType::MsgFragment(Fragment {
            fragment_index: 1,
            total_n_fragments: 1,
            length: 1,
            data: [0; 128],
        }),
        routing_header: SourceRoutingHeader {
            hop_index: 1,
            hops: [5, 1, 2, 4,6].to_vec(),
        },
        session_id: 1,
    };


    match handles_c.pop() {
        Some(c)=>{
            match c.join() {
                Ok(mut cc)=>{
                    if cc.id == 5{
                        cc.packet_send.get(&1).unwrap().send(res);
                        println!("Waiting to receive");
                        thread::sleep(Duration::from_secs(3));
                        println!("Trying to receive");
                        cc.run();
                    } else {
                    }
                },
                _=>{}
            }
        },
        None=>{}
    }
   

    while let Some(handle) = handles.pop() {
        handle.join().unwrap();
    }
}


fn crash(){

    let config = parse_config("config.toml");
    println!("{:?}", config.drone.clone());
    println!("{:?}", config.client.clone());

    let mut dd = HashMap::new();
    let mut controller_drones = HashMap::new();
    let (node_event_send, node_event_recv) = unbounded();

    let mut packet_channels = HashMap::new();
    for drone in config.drone.iter() {
        packet_channels.insert(drone.id, unbounded());
    }
    for client in config.client.iter() {
        packet_channels.insert(client.id, unbounded());
    }
    for server in config.server.iter() {
        packet_channels.insert(server.id, unbounded());
    }

    let mut handles = Vec::new();
    // let mut handles_c = Vec::new();
    for drone in config.drone.into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(drone.id, controller_drone_send);
        let node_event_send = node_event_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_node_ids
            .clone()
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();
        dd.insert(drone.id, drone.connected_node_ids.clone());
        handles.push(thread::spawn(move || {
            let mut drone = GameOfDrones::new(
                drone.id,
                node_event_send,
                controller_drone_recv,
                packet_recv,
                packet_send,
                drone.pdr,
            );
            println!("{}  {:?}", drone.id, drone.packet_send.clone());

            drone.run();
        }));
    }

    let mut c = Vec::new();
    // let mut clients = Vec::new();
    for drone in config.client.into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(drone.id, controller_drone_send);
        let node_event_send = node_event_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_drone_ids
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();

        let mut drone = Client {
            id: drone.id,
            controller_recv: controller_drone_recv,
            controller_send: node_event_send,
            packet_recv,
            packet_send,
            flood_id: 1,
        };
        c.push( drone.clone()) ;
    }

    // let mut servers = Vec::new();

    for drone in config.server.into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(drone.id, controller_drone_send);
        let node_event_send = node_event_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_drone_ids
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();

        handles.push(thread::spawn(move || {
            let mut drone = Server {
                id: drone.id,
                controller_recv: controller_drone_recv,
                controller_send: node_event_send,
                packet_recv,
                packet_send,
                flood_ids: HashSet::new(),
            };
            drone.run();
        }));
    }

    let mut controller = SimulationController {
        droness: dd,
        drones: controller_drones,
        node_event_recv,
    };

    let mut packet = Packet::new_ack(SourceRoutingHeader::with_first_hop([5,1,3,4,6].to_vec()),1,0);

    c[0].packet_send.get(&1).unwrap().send(packet.clone()).ok();
    thread::sleep(Duration::from_secs(2));
    controller.crash_all(3);
    thread::sleep(Duration::from_secs(2));

    c[0].packet_send.get(&1).unwrap().send(packet.clone()).ok();
    match c[0].packet_recv.recv() {
        Ok(p)=>{
            println!("{}",p);
        },
        _=>{}
    }
    

    while let Some(handle) = handles.pop() {
        handle.join().unwrap();
    }
}

fn nack_receiving_main(){

    let config = parse_config("config.toml");
    println!("{:?}", config.drone.clone());
    println!("{:?}", config.client.clone());

    let mut dd = HashMap::new();
    let mut controller_drones = HashMap::new();
    let (node_event_send, node_event_recv) = unbounded();

    let mut packet_channels = HashMap::new();
    for drone in config.drone.iter() {
        packet_channels.insert(drone.id, unbounded());
    }
    for client in config.client.iter() {
        packet_channels.insert(client.id, unbounded());
    }
    for server in config.server.iter() {
        packet_channels.insert(server.id, unbounded());
    }

    let mut handles = Vec::new();
    let mut handles_c = Vec::new();
    for drone in config.drone.into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(drone.id, controller_drone_send);
        let node_event_send = node_event_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_node_ids
            .clone()
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();
        dd.insert(drone.id, drone.connected_node_ids.clone());
        handles.push(thread::spawn(move || {
            let mut drone = GameOfDrones::new(
                drone.id,
                node_event_send,
                controller_drone_recv,
                packet_recv,
                packet_send,
                drone.pdr,
            );
            println!("{}  {:?}", drone.id, drone.packet_send.clone());

            drone.run();
        }));
    }

    // let mut clients = Vec::new();
    for drone in config.client.into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(drone.id, controller_drone_send);
        let node_event_send = node_event_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_drone_ids
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();

        let mut drone = Client {
            id: drone.id,
            controller_recv: controller_drone_recv,
            controller_send: node_event_send,
            packet_recv,
            packet_send,
            flood_id: 1,
        };
        handles_c.push(thread::spawn(|| drone));
    }

    // let mut servers = Vec::new();

    for drone in config.server.into_iter() {
        // controller
        let (controller_drone_send, controller_drone_recv) = unbounded();
        controller_drones.insert(drone.id, controller_drone_send);
        let node_event_send = node_event_send.clone();
        // packet
        let packet_recv = packet_channels[&drone.id].1.clone();
        let packet_send = drone
            .connected_drone_ids
            .into_iter()
            .map(|id| (id, packet_channels[&id].0.clone()))
            .collect();

        handles.push(thread::spawn(move || {
            let mut drone = Server {
                id: drone.id,
                controller_recv: controller_drone_recv,
                controller_send: node_event_send,
                packet_recv,
                packet_send,
                flood_ids: HashSet::new(),
            };
            drone.run();
        }));
    }

    let mut controller = SimulationController {
        droness: dd,
        drones: controller_drones,
        node_event_recv,
    };

    let res = Packet {
        pack_type: PacketType::MsgFragment(Fragment {
            fragment_index: 1,
            total_n_fragments: 1,
            length: 1,
            data: [0; 128],
        }),
        routing_header: SourceRoutingHeader {
            //modify here based on which Nack Packet you want to get back
            hop_index: 3,
            hops: [5, 1, 2, 4,6].to_vec(),
        },
        session_id: 1,
    };


    match handles_c.pop() {
        Some(c)=>{
            match c.join() {
                Ok(mut cc)=>{
                    if cc.id == 5{
                        cc.packet_send.get(&1).unwrap().send(res);
                        println!("Waiting to receive");
                        thread::sleep(Duration::from_secs(3));
                        println!("Trying to receive");
                        cc.run();
                    } else {
                    }
                },
                _=>{}
            }
        },
        None=>{}
    }


    while let Some(handle) = handles.pop() {
        handle.join().unwrap();
    }
}