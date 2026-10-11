# Community tools and docs (researched 2026-10-11)

Relevance = why it matters to the Rust rewrite. Licenses as advertised
upstream; re-check before vendoring any code. Nothing below is vendored.

| Resource | URL | License | Relevance |
|---|---|---|---|
| ai-game-modding-guides (trevaintdead) | https://github.com/trevaintdead/ai-game-modding-guides | MIT | Process rules for agent-written Rust rewrites; whitelist `.gitignore`, runtime-read pattern, Bevy/wgpu notes |
| IW4L (MW2 Rust/Bevy runtime) | https://github.com/vladtrc/iw4L | UNKNOWN (check repo) | Reference structure: runtime reads of the player's install, wgpu rendering, D3D9→WGSL notes |
| Soul Spiral (Ben Lincoln) + mirror | https://thelostworlds.net/Software/Soul_Spiral.html / https://github.com/TheSerioliOfNosgoth/SoulSpiral-Official | Check upstream (mirror used with permission) | Unpacks Crystal Dynamics bigfile.dat (Gex-engine family: SR1/SR2/TR Legend); first tool when a slice needs BIGFILE contents |
| ModelEx (TheSerioliOfNosgoth) | https://github.com/TheSerioliOfNosgoth/ModelEx | See repo LICENSE | Views/exports SR1/SR2/Defiance models to Collada; documents .drm/.pcm expectations; toolchain reference, not vendored |
| Soul Reaver → Blender toolchain (thelostworlds) | https://thelostworlds.net/Software/Soul_Reaver_to_Blender_Conversion_Toolchain.html | Fan toolchain (check page) | Remaster-model path into Blender; cited by Nexus model mods as guidance |
| Recombobulator | https://github.com/TheSerioliOfNosgoth/Recombobulator | Check repo | Analyses SR1 room/object files; useful when the engine slice reaches maps |
| AsunderNoMore | https://github.com/TheSerioliOfNosgoth/AsunderNoMore | Check repo | Packs SR assets into new bigfile.dat; shows archive write path (we only read) |
| Only Skin Deep / SR PC Texture Utility (mirrors) | https://github.com/TheSerioliOfNosgoth (org) | Used with permission (Ben Lincoln) | Legacy texture extraction reference for the 1999 PC build |
| LOK Noesis plugins | https://github.com/TheSerioliOfNosgoth/LOK-NoesisPlugins | Check repo | Independent model-format readers to cross-check structs |
| unLoKable (SalsaGal) | https://github.com/SalsaGal/unLoKable | MIT | Rust suite for Crystal Dynamics audio; `desnd` decompiles paired `.SND`+`.SMP` (we have 31 pairs under `1\MUSIC\`) into VH/VB + sequences |
| SoundReaver 2 + PSound (how-to) | https://thelostworlds.net/HowTo/Extract_Soul_Reaver_2_and_Blood_Omen_2_Audio_PS2.html | Free tools (check terms) | PS2-era SR2/BO2 audio extraction path |
| soul-re (fmil95) | https://github.com/fmil95/soul-re | MIT | PSX SR1 MIPS→C decomp (prototype with debug symbols); format/behavior reference only, never vendored |
| KAIN2 (Gh0stBlade) | https://github.com/Gh0stBlade/KAIN2 | Check repo | SR1 decomp/port effort (Valkyrie/SDL2); behavior reference only, never vendored |
| Nexus: SR2 Tweaked Character Models (vargatomi) | https://www.nexusmods.com/legacyofkainsoulreaver1and2remastered/mods/40 | Mod page terms | Proves remaster meshes/textures are community-editable via Blender; install = overwrite in game folder (we never do that) |
| XeNTaX wiki (bigfile RE notes) | https://wiki.xentax.spektr.name | Wiki (varies) | No SR-specific bigfile spec found; Soul Spiral is the authoritative community spec instead |

## Engine/file-layout question

Aspyr's own post (PlayStation.Blog, 2024-09-24) says the remaster team
started from Crystal Dynamics' original source + assets with "remaster
pillars" shared with Tomb Raider I-III Remastered — i.e. shared
philosophy (original engine, pixel-faithful + HD assets), NOT a confirmed
shared file layout. Observed remaster layout (loose `.DDS` in `TEX/`,
`*.BIG` archives, `sr1.dll`/`sr2.dll`, `.OGG` audio) is its own thing:
treat Tomb Raider tooling as UNRELATED until proven otherwise.

## XeNTaX bigfile.dat spec

No dedicated Soul Reaver bigfile.dat page was found on the XeNTaX wiki.
The community spec is Soul Spiral (above) plus its mirrored source.
Recorded as: XeNTaX = no entry found (2026-10-11), Soul Spiral = spec.
