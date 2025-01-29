use coremidi::{Client, PacketList, Source, Sources};

use std::io::{self, BufRead};
use tokio::sync::mpsc;
use uuid::Uuid;

use ble_peripheral_rust::{
    gatt::{
        characteristic::Characteristic,
        descriptor::Descriptor,
        peripheral_event::{
            PeripheralEvent, ReadRequestResponse, RequestResponse, WriteRequestResponse,
        },
        properties::{AttributePermission, CharacteristicProperty},
        service::Service,
    },
    uuid::ShortUuid,
    Peripheral, PeripheralImpl,
};

#[tokio::main]
async fn main() {
    if Sources::count() == 0 {
        eprintln!("No MIDI sources available");
        std::process::exit(-1);
    }

    let source = Source::from_index(0).unwrap();
    println!("Using MIDI source: {}", source.display_name().unwrap());

    let client = Client::new("Example Client").unwrap();

    let callback = |packet_list: &PacketList| {
        //PacketList(ptr=16fa0a608, packets=[Packet(ptr=16fa0a60c, ts=0000020ecf214ce8, data=[80, 3e, 7f])])
        println!("{:?}", packet_list);
    };

    let port = client.input_port("Example Port", callback).unwrap();

    port.connect_source(&source).unwrap();

    start_app().await;

    println!("disconnect midi");
    port.disconnect_source(&source).unwrap();
}

async fn start_app() {
    let char_uuid = Uuid::from_short(0x2A3D_u16);

    // Define Service With Characteristics
    let service = Service {
        uuid: Uuid::from_short(0x1234_u16),
        primary: true,
        characteristics: vec![
            Characteristic {
                uuid: char_uuid,
                properties: vec![
                    CharacteristicProperty::Read,
                    CharacteristicProperty::Write,
                    CharacteristicProperty::Notify,
                ],
                permissions: vec![
                    AttributePermission::Readable,
                    AttributePermission::Writeable,
                ],
                value: None,
                descriptors: vec![Descriptor {
                    uuid: Uuid::from_short(0x2A13_u16),
                    value: Some(vec![0, 1]),
                    ..Default::default()
                }],
            },
            Characteristic {
                uuid: Uuid::from_string("1209"),
                ..Default::default()
            },
        ],
    };

    let (sender_tx, mut receiver_rx) = mpsc::channel::<PeripheralEvent>(256);

    let mut peripheral = Peripheral::new(sender_tx).await.unwrap();

    // Handle Updates
    tokio::spawn(async move {
        while let Some(event) = receiver_rx.recv().await {
            handle_updates(event);
        }
    });

    while !peripheral.is_powered().await.unwrap() {}

    if let Err(err) = peripheral.add_service(&service).await {
        eprintln!("Error adding service: {}", err);
        return;
    }
    println!("Service Added");

    if let Err(err) = peripheral
        .start_advertising("RustBLE", &[service.uuid])
        .await
    {
        eprintln!("Error starting advertising: {}", err);
        return;
    }
    println!("Advertising Started");

    // Write in console to send to characteristic update to subscribed clients
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match line {
            Ok(input) => {
                println!("Writing: {input} to {char_uuid}");
                peripheral
                    .update_characteristic(char_uuid, input.into())
                    .await
                    .unwrap();
            }
            Err(err) => {
                eprintln!("Error reading from console: {}", err);
                break;
            }
        }
    }

    println!("start_app done");
}

/// Listen to all updates and respond if require
pub fn handle_updates(update: PeripheralEvent) {
    match update {
        PeripheralEvent::StateUpdate { is_powered } => {
            println!("PowerOn: {is_powered:?}")
        }
        PeripheralEvent::CharacteristicSubscriptionUpdate {
            request,
            subscribed,
        } => {
            println!("CharacteristicSubscriptionUpdate: Subscribed {subscribed} {request:?}")
        }
        PeripheralEvent::ReadRequest {
            request,
            offset,
            responder,
        } => {
            println!("ReadRequest: {request:?} Offset: {offset}");
            responder
                .send(ReadRequestResponse {
                    value: String::from("hi").into(),
                    response: RequestResponse::Success,
                })
                .unwrap();
        }
        PeripheralEvent::WriteRequest {
            request,
            offset,
            value,
            responder,
        } => {
            println!("WriteRequest: {request:?} Value: {value:?} Offset: {offset}");
            responder
                .send(WriteRequestResponse {
                    response: RequestResponse::Success,
                })
                .unwrap();
        }
    }
}
