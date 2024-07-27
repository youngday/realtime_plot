use log::{debug, error, info, trace, warn};
use log4rs;
use tokio::{task, time};

use std::error::Error;
use std::time::Duration;

use rumqttc::v5::mqttbytes::QoS;
use rumqttc::v5::{AsyncClient, MqttOptions};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    // pretty_env_logger::init();
    // color_backtrace::install();

    log4rs::init_file("examples/config/log.yaml", Default::default()).unwrap();
    info!("log start:trace,debug,info,warn,error.");

    let mut mqttoptions = MqttOptions::new("test-1", "localhost", 1884);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);
    task::spawn(async move {
        requests(client).await;
        time::sleep(Duration::from_secs(3)).await;
    });

    loop {
        let event = eventloop.poll().await;
        match &event {
            Ok(v) => {
                // info!("Event = {v:?}");//TODO:youngday:you may get sub ack from incoming
            }
            Err(e) => {
                error!("Error = {e:?}");
                return Ok(());
            }
        }
    }
}

async fn requests(client: AsyncClient) {
    // client
    //     .subscribe("hello/world", QoS::AtMostOnce)
    //     .await
    //     .unwrap();
    let mut i = 0;
    let mut send_vec=vec![0,1,i];
    info!("send_vec:{:?}",i);
    loop {
        client
            .publish("hello", QoS::ExactlyOnce, false, send_vec.clone())
            .await
            .unwrap();
        send_vec=vec![0,1,i];
        if i > 100 {
            i = 0;
        }
        info!("pub vec:{:?}",i);
        i+=1;

        time::sleep(Duration::from_secs_f64(0.029)).await;
    }
}
