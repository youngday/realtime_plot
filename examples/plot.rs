use futures::future::ok;
use piston_window::{EventLoop, PistonWindow, WindowSettings};
use plotters::prelude::*;
use systemstat::platform::common::Platform;
use systemstat::System;

use crate::mpsc::Sender;
use chrono::prelude::*;
use log::{debug, error, info, trace, warn};
use log4rs;
use std::collections::vec_deque::VecDeque;
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio::{task, time};

use realtime_plot::draw_piston_window;
use realtime_plot::Settings;

use futures::StreamExt;
use tmq::{subscribe, Context, Result};

use iceoryx2::{port::subscriber, prelude::*};
use realtime_plot::transmission_data::TransmissionData;

use rumqttc::v5::mqttbytes::v5::Packet;
use rumqttc::v5::mqttbytes::v5::Packet::Publish;
use rumqttc::v5::mqttbytes::v5::Packet::SubAck;
use rumqttc::v5::mqttbytes::QoS;
use rumqttc::v5::{
    AsyncClient,
    Event::{Incoming, Outgoing},
    MqttOptions,
};

const CYCLE_TIME: Duration = Duration::from_millis(10);

const FPS: u32 = 60;
const LENGTH: u32 = 5;
const N_DATA_POINTS: usize = (FPS * LENGTH) as usize;
const COM_TYPE: u32 = 1; //0=zeromq 1=ice_oryx2,2=mqtt?
#[tokio::main]
async fn main() {
    let mut window: PistonWindow = WindowSettings::new("Real Time CPU Usage", [450, 300])
        .samples(4)
        .build()
        .unwrap();

    log4rs::init_file("examples/config/log.yaml", Default::default()).unwrap();
    let settings = Settings::new();
    info!("{:?}", settings);
    let version: String = "0.1.0703".to_string();
    info!("Start your app,version:{0}", version);

    // let (tx, mut rx) = mpsc::channel(100);

    let (sender, receiver) = channel();

    let mut loop_cnt: i64 = 0;

    let mut socket: subscribe::Subscribe = subscribe(&Context::new())
        .connect("tcp://127.0.0.1:7899")
        .unwrap()
        .subscribe(b"topic")
        .unwrap();

    //zeromq received

    if COM_TYPE == 0 {
        tokio::spawn(async move {
            loop {
                info!("loop_cnt {:?}", loop_cnt);
                loop_cnt += loop_cnt;
                let now = Instant::now(); // 程序起始时间
                info!("zmq_sub start: {:?}", now);
                let val = zmq_sub(&mut socket).await.unwrap();
                let end = now.elapsed().as_millis();
                info!("zmq_sub end,dur: {:?} ms.", end);
                let ret_send = sender.send(val);
                info!("ret_send: {:?}", ret_send);
                info!("🟢 send val: {:?}", val);
            }
        });
    } else if COM_TYPE == 1 {
        //iceoryx2 received
        tokio::spawn(async move {
            //for spawn , merge:change ? into unwrap()
            let node = NodeBuilder::new().create::<ipc::Service>().unwrap();
            let service = node
                .service_builder(&"My/Funk/ServiceName".try_into().unwrap())
                .publish_subscribe::<TransmissionData>()
                .open_or_create()
                .unwrap();

            let subscriber = service.subscriber_builder().create().unwrap();

            while node.wait(CYCLE_TIME).is_ok() {
                while let Some(sample) = subscriber.receive().unwrap() {
                    println!("received: {:?}", *sample);
                    //plot interface
                    let val = sample.x.as_f64() * 0.01;
                    let _ret_send = sender.send(val);
                    info!("🟢 send val: {:?}", val);
                }
            }
            info!("exit ...");
        });
        // let result = computation.join().unwrap();//TODO: block and nonblock
    } else if COM_TYPE == 2 {
        let mut mqttoptions = MqttOptions::new("test-2", "localhost", 1884);
        mqttoptions.set_keep_alive(Duration::from_secs(5));

        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
        task::spawn(async move {
            requests(client).await;
            time::sleep(Duration::from_secs(3)).await;
        });

        task::spawn(async move {
            loop {
                let now = Instant::now(); // 程序起始时间
                info!("mqtt start: {:?}", now);
                let event = eventloop.poll().await;
                match &event {
                    Ok(v) => {
                        // info!("Event = {v:?}");
                        match &v {
                            Incoming(i) => {
                                // info!("incoming = {i:?}");
                                match &i {
                                    Packet::Connect(_, _, _) => {}
                                    Packet::ConnAck(_) => {}
                                    Publish(p) => {
                                        // info!("publish = {p:?}");
                                        let _topic = p.topic.clone();
                                        let _payload = p.payload.clone();
                                        let val = _payload.as_ref()[2].as_f64() * 0.01; //[0,1,val]
                                        info!(
                                            "\ntopic = {0:?},payload = {1:?}",
                                            _topic,
                                            _payload.as_ref()
                                        );

                                        let end = now.elapsed().as_millis();
                                        info!("mqtt end,dur: {:?} ms.", end);
                                        let ret_send = sender.send(val);
                                        info!("ret_send: {:?}", ret_send);
                                        info!("🟢 send val: {:?}", val);
                                    }
                                    Packet::PubAck(_) => {}
                                    Packet::PingReq(_) => {}
                                    Packet::PingResp(_) => {}
                                    Packet::Subscribe(_) => {}
                                    SubAck(ack) => {
                                        info!("ack = {ack:?}");
                                    }
                                    Packet::PubRec(_) => {}
                                    Packet::PubRel(_) => {}
                                    Packet::PubComp(_) => {}
                                    Packet::Unsubscribe(_) => {}
                                    Packet::UnsubAck(_) => {}
                                    Packet::Disconnect(_) => {}
                                }
                            } //pack
                            Outgoing(_o) => {
                                // info!("Outgoing = {o:?}");
                            }
                        } //event
                    } //ok
                    Err(e) => {
                        error!("Error = {e:?}");
                        // return Ok(());
                    } //err
                } //result
            } //loop
        });
    } else {
    }

    let sys = System::new();
    window.set_max_fps(FPS as u64);
    let mut load_measurement: Vec<_> = (0..FPS).map(|_| sys.cpu_load().unwrap()).collect();
    let mut epoch = 0;
    let mut data = vec![];
    let mut my_data: f64 = 0.0;
    while let Some(_) = draw_piston_window(&mut window, |b| {
        let now = Instant::now(); // 程序起始时间
        info!("gui start: {:?}", now);

        let mut cpu_loads = load_measurement[epoch % FPS as usize].done()?;
        load_measurement[epoch % FPS as usize] = sys.cpu_load()?; //

        // let rx_data = rx.try_recv();//zeromq
        let rx_data = receiver.recv(); //ice_oryx
        info!("🟡 receive {:?}", rx_data);

        if rx_data.is_ok() {
            my_data = rx_data.unwrap();
            info!("🔴 receive {:?}", my_data);
        }
        info!("plot {:?}", my_data);
        cpu_loads[0].idle = my_data as f32;
        // println!("cpu_loads:{0:?}\n", cpu_loads);
        let root = b.into_drawing_area();
        root.fill(&WHITE)?;
        if data.len() < cpu_loads.len() {
            for _ in data.len()..cpu_loads.len() {
                data.push(VecDeque::from(vec![0f32; N_DATA_POINTS + 1]));
            }
        }
        // println!("data:{0:?}\n", data);
        for (core_load, target) in cpu_loads.into_iter().zip(data.iter_mut()) {
            if target.len() == N_DATA_POINTS + 1 {
                target.pop_front();
            }
            target.push_back(1.0 - core_load.idle);
        }
        let end = now.elapsed().as_millis();
        info!("⏰ 🪟 gui end,dur 5: {:?} ms.", end);
        let mut cc = ChartBuilder::on(&root)
            .margin(10)
            .caption("Real Time CPU Usage", ("sans-serif", 30))
            .x_label_area_size(40)
            .y_label_area_size(50)
            .build_cartesian_2d(0..N_DATA_POINTS as u32, 0f32..1f32)?;
        let end = now.elapsed().as_millis();
        info!("⏰ 🪟 gui end,dur 6 : {:?} ms.", end); //4
        cc.configure_mesh()
            .x_label_formatter(&|x| format!("{}", -(LENGTH as f32) + (*x as f32 / FPS as f32)))
            .y_label_formatter(&|y| format!("{}%", (*y * 100.0) as u32))
            .x_labels(15)
            .y_labels(5)
            .x_desc("Seconds")
            .y_desc("% Busy")
            .axis_desc_style(("sans-serif", 15))
            .draw()?;
        let end = now.elapsed().as_millis();
        info!("⏰ 🪟 gui end,dur 7: {:?} ms.", end); //7
        for (idx, data) in (0..2).zip(data.iter()) {
            //✏️  in(0..) to in(0..x) plot number
            cc.draw_series(LineSeries::new(
                (0..).zip(data.iter()).map(|(a, b)| (a, *b)),
                &Palette99::pick(idx),
            ))?
            .label(format!("CPU {}", idx))
            .legend(move |(x, y)| {
                Rectangle::new([(x - 5, y - 5), (x + 5, y + 5)], &Palette99::pick(idx))
            });
        }
        let end = now.elapsed().as_millis();
        info!("⏰ 🪟 gui end,dur 8: {:?} ms.", end); //24  2*plot_number  2*12=24ms
        cc.configure_series_labels()
            .background_style(&WHITE.mix(0.8))
            .border_style(&BLACK)
            .draw()?;

        epoch += 1;
        let end = now.elapsed().as_millis();
        info!("⏰ 🪟 gui end,dur: {:?} ms.", end); //4
        Ok(())
    }) {}
}

async fn zmq_sub(socket: &mut subscribe::Subscribe) -> Result<f64> {
    let mut value: f64 = 0.0;
    let mut now = Instant::now(); // 程序起始时间
    let optional = socket.next().await;
    info!("zmq_sub p1,dur: {:?} ms.", now.elapsed().as_millis());
    now = Instant::now(); // 程序起始时间

    match optional {
        // 如果 `optional` 解构成功，就执行下面语句块。
        Some(msg) => {
            let mut ix: i32 = 0;
            match IntoIterator::into_iter(msg?) {
                mut iter => loop {
                    let next;
                    match iter.next() {
                        Some(val) => next = val,
                        None => break,
                    };
                    let x = next.as_str();
                    match x {
                        Some(p) => {
                            if ix == 0 {
                                // info!("topic:{:?}", p);
                            } else if ix == 1 {
                                value = p.parse::<f64>().unwrap();
                                // info!("value:{:?}", value);
                            };
                            // println!("has value {p}")
                        }
                        None => break,
                    }
                    ix += 1;
                },
            };
        }
        // 当解构失败时退出循环：
        _ => {} // ^ 为什么必须写这样的语句呢？肯定有更优雅的处理方式！
    }
    Ok(value)
}

async fn requests(client: AsyncClient) {
    loop {
        client.subscribe("hello", QoS::AtMostOnce).await.unwrap();
        time::sleep(Duration::from_secs(1)).await;
        // info!("subscribe:");
    }

    // time::sleep(Duration::from_secs(120)).await;
}
