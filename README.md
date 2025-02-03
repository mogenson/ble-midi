# ble-midi
Read MIDI from a CoreMidi port and write it to a BLE characteristic. MacOS only.

This program opens the first available MIDI input port. It assumes only one keyboard is connected to the computer.

Next, it scans for a BLE peripheral by name. The name is currently hardcoded to "CH-8" for a Teenage Engineering Choir doll. The default pairing code is 000000.

![Teenage Engineering Choir doll](https://teenage.engineering/_img/636bb6605334794ec4ee5dfa_512.png)

Then, MIDI notes from the keyboard are written to the MIDI characteristic on the BLE peripheral. Pressing Ctrl-C exits the program.
