use coremidi::{Client, PacketList, Source, Sources};

use tokio::signal;
use tokio::sync::mpsc::{self, Receiver};
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

    let (tx, rx) = mpsc::channel(1);

    let source = Source::from_index(0).unwrap();
    println!("Using MIDI source: {}", source.display_name().unwrap());

    let client = Client::new("Example Client").unwrap();

    let callback = move |packet_list: &PacketList| {
        //PacketList(ptr=16fa0a608, packets=[Packet(ptr=16fa0a60c, ts=0000020ecf214ce8, data=[80, 3e, 7f])])
        for packet in packet_list.iter() {
            println!("Sending {:?}", packet);
            tx.blocking_send(packet.data().to_vec()).unwrap();
        }
    };

    let port = client.input_port("Example Port", callback).unwrap();

    port.connect_source(&source).unwrap();

    let ctrl_c = async {
        signal::ctrl_c().await.unwrap();
    };

    tokio::select! {
        _ = ctrl_c => {
            println!("Ctrl-C received, exiting!");
        }
        _ = ble_task(rx) => {
            println!("BLE task completed.");
        }
    }

    port.disconnect_source(&source).unwrap();
}

async fn ble_task(mut rx: Receiver<Vec<u8>>) {
    let char_uuid = Uuid::parse_str("7772E5DB-3868-4112-A1A9-F2669D106BF3").unwrap();

    let service = Service {
        uuid: Uuid::parse_str("03B80E5A-EDE8-4B33-A751-6CE34EC4C700").unwrap(),
        primary: true,
        characteristics: vec![Characteristic {
            uuid: char_uuid,
            properties: vec![
                CharacteristicProperty::Read,
                CharacteristicProperty::WriteWithoutResponse,
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
        }],
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
        .start_advertising("BLEMIDI", &[service.uuid])
        .await
    {
        eprintln!("Error starting advertising: {}", err);
        return;
    }
    println!("Advertising Started");

    loop {
        let data = rx.recv().await.unwrap();
        println!("Received {:?}", data);

        // MSb high for header byte and timestamp byte
        // Timestamp value is always zero
        let mut midi_data = vec![0x80, 0x80];
        midi_data.extend(data);

        peripheral
            .update_characteristic(char_uuid, midi_data)
            .await
            .unwrap();
    }
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
