# DAIA — Debian AI Appliance

DAIA is an open-source platform for creating ready-to-use, Debian-based appliances.

Imagine having a computer that does exactly what you need—whether it's a personal AI assistant, an offline knowledge library, a mail server, or a home storage system—without having to install and configure everything yourself.

That's what DAIA aims to make possible.

Instead of building a different operating system for every purpose, DAIA lets you choose the capabilities you need and brings them together into a dedicated appliance.

## What Can You Build with DAIA?

DAIA is designed to support a growing family of appliances:

- **AI:** Local AI assistants, Ollama, Open WebUI, speech recognition, and computer vision.
- **Wanderer:** Offline maps, Wikipedia, navigation, software-defined radio (SDR), and AI—useful even without an internet connection.
- **Mail:** Your own email server with Postfix, Dovecot, OpenLDAP, administration, and monitoring.
- **Storage:** Personal or business storage with Samba, NFS, RAID, ZFS, and backups.
- **Cloud:** Private cloud services with Nextcloud, file synchronization, and collaboration.
- **Networking:** DNS, DHCP, VPN, and firewall appliances.
- **Security:** System monitoring, intrusion detection, and auditing.

These are examples of DAIA's intended capabilities. Not all are implemented yet.

## One Platform. Many Possibilities.

The idea behind DAIA is simple: **you decide what your appliance should do, and DAIA takes care of putting the pieces together.**

Want an AI workstation? A private family cloud? An offline travel companion? Or a combination of several capabilities?

DAIA is being built to make these possible without maintaining a separate project for each appliance.

And as new technologies become available, new capabilities can be added and reused across different appliances.

## Built on Debian. Powered by Rust.

DAIA combines Debian's stability with a modular, Rust-based engine and installer.

Its goal is to make building and installing specialized systems simpler, more consistent, and easier to reproduce.

## Project Status

DAIA is under active development. Its Debian-based image builder, Rust engine, capability framework, and installer are being developed and tested. Additional appliance capabilities will be introduced progressively.

## Our Vision

**One engine. One installer. Many appliances.**

DAIA isn't about creating yet another Linux distribution.

It's about making it easier for anyone to turn an idea into a purpose-built appliance.
