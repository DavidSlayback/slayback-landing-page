#!/usr/bin/env bash
# Imports the game's recorded sound effects from Kenney's "Impact Sounds" pack (CC0, https://kenney.nl/assets/impact-sounds;
# no credit required, credited anyway in crates/game3d/assets/sfx/README.md): wooden knocks for blocks being hit and
# landing (decision Q79). Usage, with the pack unzipped somewhere:
#   bash tools/sfx/import.sh <unzipped pack dir>
# Writes 16-bit mono 44.1 kHz WAVs to crates/game3d/assets/sfx/, each trimmed of leading silence and faded out at its
# end. Needs ffmpeg. The synthesised effects (whoosh, boom) come from tools/sfx/generate.py.
set -euo pipefail
pack="${1:?usage: import.sh <unzipped Kenney Impact Sounds dir>}"
out="$(cd "$(dirname "$0")/../.." && pwd)/crates/game3d/assets/sfx"
mkdir -p "$out"
find_one() { find "$pack" -name "$1.ogg" | head -1; }
# Leading silence trimmed, and a 20 ms fade at the end so a cut tail never clicks (a fade-in on the reversed sound, so
# it needs no duration).
shape='silenceremove=start_periods=1:start_threshold=-50dB,areverse,afade=t=in:d=0.02,areverse'
convert() {
  ffmpeg -v error -y -i "$(find_one "$1")" -ac 1 -ar 44100 -sample_fmt s16 -af "$shape" "$out/$2.wav"
}
# name in the pack -> name in the game
convert impactWood_medium_000 hit_0
convert impactWood_medium_002 hit_1
convert impactWood_medium_004 hit_2
convert impactWood_heavy_000 heavy_0
convert impactWood_heavy_002 heavy_1
convert impactWood_heavy_004 heavy_2
convert impactWood_light_000 clack_0
convert impactWood_light_002 clack_1
convert impactWood_light_004 clack_2
convert impactGeneric_light_001 clack_3
convert impactPlank_medium_001 crack
# The door's warning: knock, knock.
ffmpeg -v error -y -i "$(find_one impactWood_medium_001)" -i "$(find_one impactWood_medium_003)" -filter_complex \
  "[0]aformat=channel_layouts=mono,atrim=0:0.16,apad=pad_dur=0.06[a];[1]aformat=channel_layouts=mono[b];[a][b]concat=n=2:v=0:a=1,$shape" \
  -ac 1 -ar 44100 -sample_fmt s16 "$out/door.wav"
echo "imported into $out"
