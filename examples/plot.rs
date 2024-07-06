use piston_window::{EventLoop, PistonWindow, WindowSettings};
use plotters::prelude::*;
use systemstat::platform::common::Platform;
use systemstat::System;

use chrono::prelude::*;
use log::{debug, error, info, trace, warn};
use log4rs;
use std::collections::vec_deque::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio::time::sleep;

use realtime_plot::draw_piston_window;
use realtime_plot::Settings;

use futures::StreamExt;
use tmq::{subscribe, Context, Result};

const FPS: u32 = 15;
const LENGTH: u32 = 20;
const N_DATA_POINTS: usize = (FPS * LENGTH) as usize;

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

    let (tx, mut rx) = mpsc::channel(100);
    let mut loop_cnt: i64 = 0;

    let mut socket: subscribe::Subscribe = subscribe(&Context::new())
        .connect("tcp://127.0.0.1:7899")
        .unwrap()
        .subscribe(b"topic")
        .unwrap();

    tokio::spawn(async move {
        loop {
            // info!("loop_cnt {:?}", loop_cnt);
            loop_cnt += loop_cnt;
            let now = Instant::now(); // 程序起始时间
            info!("zmq_sub start: {:?}", now);
            let val = zmq_sub(&mut socket).await.unwrap();
            let end = now.elapsed().as_millis();
            info!("zmq_sub end,dur: {:?} ms.", end);
            let ret_send = tx.send(val).await;
            info!("ret_send: {:?}", ret_send);
            info!("🟢 send val: {:?}", val);
        }
    });

    let sys = System::new();
    window.set_max_fps(FPS as u64);
    let mut load_measurement: Vec<_> = (0..FPS).map(|_| sys.cpu_load().unwrap()).collect();
    let mut epoch = 0;
    let mut data = vec![];
    let mut my_data: f64 = 0.0;
    while let Some(_) = draw_piston_window(&mut window, |b| {
        let mut cpu_loads = load_measurement[epoch % FPS as usize].done()?;

        load_measurement[epoch % FPS as usize] = sys.cpu_load()?; //

        let rx_data = rx.try_recv();
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

        let mut cc = ChartBuilder::on(&root)
            .margin(10)
            .caption("Real Time CPU Usage", ("sans-serif", 30))
            .x_label_area_size(40)
            .y_label_area_size(50)
            .build_cartesian_2d(0..N_DATA_POINTS as u32, 0f32..1f32)?;

        cc.configure_mesh()
            .x_label_formatter(&|x| format!("{}", -(LENGTH as f32) + (*x as f32 / FPS as f32)))
            .y_label_formatter(&|y| format!("{}%", (*y * 100.0) as u32))
            .x_labels(15)
            .y_labels(5)
            .x_desc("Seconds")
            .y_desc("% Busy")
            .axis_desc_style(("sans-serif", 15))
            .draw()?;

        for (idx, data) in (0..).zip(data.iter()) {
            cc.draw_series(LineSeries::new(
                (0..).zip(data.iter()).map(|(a, b)| (a, *b)),
                &Palette99::pick(idx),
            ))?
            .label(format!("CPU {}", idx))
            .legend(move |(x, y)| {
                Rectangle::new([(x - 5, y - 5), (x + 5, y + 5)], &Palette99::pick(idx))
            });
        }

        cc.configure_series_labels()
            .background_style(&WHITE.mix(0.8))
            .border_style(&BLACK)
            .draw()?;

        epoch += 1;
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
