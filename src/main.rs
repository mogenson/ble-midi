use btleplug::{
    api::{Central, Manager as _, Peripheral as _, ScanFilter, WriteType},
    platform::{Adapter, Manager, Peripheral},
};
use coremidi::{Client, PacketList, Source, Sources};
use std::{error::Error, time::Duration};
use tokio::{
    signal,
    sync::mpsc::{self, Receiver},
    time,
};
use uuid::Uuid;

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
        result = ble_task(rx) => {
            if let Err(error) = result {
                eprintln!("ble task error: {}", error);
            }
            println!("BLE task completed.");
        }
    }

    // todo disconnect and forget
    port.disconnect_source(&source).unwrap();
}

async fn ble_task(mut rx: Receiver<Vec<u8>>) -> Result<(), Box<dyn Error>> {
    let midi_uuid = Uuid::parse_str("7772E5DB-3868-4112-A1A9-F2669D106BF3")?;
    let manager = Manager::new().await.unwrap();
    let adapters = manager.adapters().await?;
    let central = adapters.into_iter().nth(0).ok_or("can't get adapter")?;
    central.start_scan(ScanFilter::default()).await?;
    time::sleep(Duration::from_secs(4)).await;
    let peripheral = find_by_name(&central, "CH-8")
        .await
        .ok_or("can't find peripehral")?;
    peripheral.connect().await?;
    peripheral.discover_services().await?;
    let chars = peripheral.characteristics();
    let midi_char = chars
        .iter()
        .find(|c| c.uuid == midi_uuid)
        .ok_or("can't find midi characteristic")?;

    loop {
        let data = rx.recv().await.unwrap();
        println!("Received {:?}", data);

        // MSb high for header byte and timestamp byte
        // Timestamp value is always zero
        let mut midi_data = vec![0x80, 0x80];
        midi_data.extend(data);

        peripheral
            .write(&midi_char, &midi_data, WriteType::WithoutResponse)
            .await?;
    }
}

async fn find_by_name(central: &Adapter, name: &str) -> Option<Peripheral> {
    for p in central.peripherals().await.unwrap() {
        if p.properties()
            .await
            .unwrap()
            .unwrap()
            .local_name
            .iter()
            .any(|string| string.contains(name))
        {
            return Some(p);
        }
    }
    None
}
