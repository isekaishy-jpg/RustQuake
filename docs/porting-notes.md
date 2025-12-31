# Porting Notes

Focus: GLQuake + GLQuakeWorld (OpenGL renderers) from the id-Software Quake source.

Target: one Rust client that supports:
- Singleplayer (GLQuake / NetQuake rules + id1 content).
- Online play (GLQuakeWorld / QuakeWorld protocol).

Renderer scope: OpenGL only (software renderer is out of scope).

Planned Rust outputs:
- crates/qw-client (unified client binary, QW + singleplayer modes)
- crates/qw-server (QuakeWorld server binary)
- crates/qw-common (shared code)

Upstream roots (GL focus):
- vendor/quake/WinQuake (GLQuake sources: gl_*.c, gl_*.h + client/server glue)
- vendor/quake/QW/client (GLQuakeWorld sources: gl_*.c, gl_*.h + QW client)
- vendor/quake/QW/server (QuakeWorld server)

Current groundwork (QW path):
- Data path discovery + local asset lookup (PAK/WAD/BSP header parsing)
- Message buffer read/write + protocol constants + netchan header/packet logic
- Server message parsing (serverdata, print, stufftext, sound/modellists)
- Packet entities parsing (svc_packetentities/svc_deltapacketentities) + baseline parsing
- Client move message builder (clc_move + checksum + delta request) + stringcmd helper
- Client runner send path for clc_move/clc_stringcmd with delta tracking
- Signon scaffolding: auto requests soundlist/modellist and prespawn after serverdata
- Added parsing for common svc messages (setview/setangle/lightstyle/sound/stats/etc.)
- Download/chokecount/nails parsing stubs to keep stream aligned
- Info string helpers and COM_Parse tokenizer
- Client handshake scaffolding (getchallenge/connect helpers + UDP loopback tests)
- Client state model for userinfo/serverinfo + player updates + scoreboard fields + packet entity frame apply
