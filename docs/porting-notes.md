# Porting Notes

Focus: QuakeWorld in vendor/quake/QW.

Planned Rust outputs:
- crates/qw-client (client binary)
- crates/qw-server (server binary)
- crates/qw-common (shared code)

Current groundwork:
- Data path discovery + local asset lookup (PAK/WAD/BSP header parsing)
- Message buffer read/write + protocol constants + netchan header/packet logic
- Server message parsing (serverdata, print, stufftext, sound/modellists)
- Packet entities parsing (svc_packetentities/svc_deltapacketentities) + baseline parsing
- Client move message builder (clc_move + checksum + delta request) + stringcmd helper
- Client runner send path for clc_move/clc_stringcmd with delta tracking
- Info string helpers and COM_Parse tokenizer
- Client handshake scaffolding (getchallenge/connect helpers + UDP loopback tests)
- Client state model for userinfo/serverinfo + player updates + scoreboard fields + packet entity frame apply
