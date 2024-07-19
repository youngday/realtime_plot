use log::{debug, error, info, trace, warn};
use log4rs;

use rumqttc::tokio_rustls::rustls::internal::msgs::base::Payload;
use tmq::publish;
use tokio::{task, time};

use std::error::Error;
use std::time::Duration;

use rumqttc::v5::mqttbytes::v5::Packet;
use rumqttc::v5::mqttbytes::v5::Packet::Publish;
use rumqttc::v5::mqttbytes::v5::Packet::SubAck;
use rumqttc::v5::mqttbytes::QoS;
use rumqttc::v5::{
    AsyncClient,
    Event::{Incoming, Outgoing},
    MqttOptions,
};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    // pretty_env_logger::init();
    // color_backtrace::install();

    log4rs::init_file("examples/config/log.yaml", Default::default()).unwrap();
    info!("log start:trace,debug,info,warn,error.");

    let mut mqttoptions = MqttOptions::new("test-2", "localhost", 1884);
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
                // info!("Event = {v:?}");
                match &v {
                    Incoming(i) => {
                        // info!("incoming = {i:?}");
                        match &i {
                            Packet::Connect(_, _, _) => {}
                            Packet::ConnAck(_) => {}
                            Publish(p) => {
                                // info!("publish = {p:?}");
                                let _topic=p.topic.clone();
                                let _payload=p.payload.clone();
                                info!("\ntopic = {0:?},payload = {1:?}",_topic,_payload.as_ref());

                     

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
                    }
                    Outgoing(o) => {
                        // info!("Outgoing = {o:?}");
                    }
                }
            }
            Err(e) => {
                error!("Error = {e:?}");
                return Ok(());
            }
        }
    }
}

async fn requests(client: AsyncClient) {
    loop {
        client.subscribe("hello", QoS::AtMostOnce).await.unwrap();
        time::sleep(Duration::from_secs(1)).await;
        // info!("subscribe:");
    }

    // time::sleep(Duration::from_secs(120)).await;
}
