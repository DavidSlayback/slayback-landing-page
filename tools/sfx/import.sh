#!/usr/bin/env bash
# Imports the game's recorded sound effects (decision Q79): David's picks, by ear, from CC0 packs on OpenGameArt and
# Kenney (sources and credits in crates/game3d/assets/sfx/README.md). Usage, with the packs unzipped into one folder:
#   <dir>/punches   "37 hits/punches" (Independent.nu)       independent_nu_ljudbank-hits_and_punches.7z
#   <dir>/breaking  "75 CC0 breaking/falling/hit sfx" (rubberduck)   sfx_breaking_and_falling.zip
#   <dir>/kenney    Kenney "Impact Sounds"                    kenney_impact-sounds.zip
#   bash tools/sfx/import.sh <dir>
# Writes 16-bit mono 44.1 kHz WAVs to crates/game3d/assets/sfx/: leading silence trimmed, peak-normalised to -1 dBFS
# (the packs were recorded at very different levels, some past full scale), faded out over the last 20 ms. Needs ffmpeg
# (a .7z opens with Windows' own tar). The synthesised effects (whoosh, boom) come from tools/sfx/generate.py.
set -euo pipefail
packs="${1:?usage: import.sh <dir holding the unzipped packs>}"
out="$(cd "$(dirname "$0")/../.." && pwd)/crates/game3d/assets/sfx"
mkdir -p "$out"
find_one() {
  local f
  f="$(find "$packs/$1" -name "$2.*" | head -1)"
  [ -n "$f" ] || { echo "not found: $1/$2" >&2; exit 1; }
  echo "$f"
}
# Trims silence at both ends, then fades the end out (both on the reversed sound, so neither needs the duration).
shape='silenceremove=start_periods=1:start_threshold=-50dB,areverse,silenceremove=start_periods=1:start_threshold=-50dB,afade=t=in:d=0.02,areverse'
# Converts with float headroom, measures the peak, then writes it normalised to -1 dBFS.
convert() {
  local src="$1" name="$2" tmp="$out/.$2.f32.wav" peak
  ffmpeg -v error -y -i "$src" -ac 1 -ar 44100 -c:a pcm_f32le -af "$shape" "$tmp"
  # astats, not volumedetect: it reads float peaks past full scale.
  peak="$(ffmpeg -v info -i "$tmp" -af astats -f null - 2>&1 | sed -n 's/.*Peak level dB: \(-\?[0-9.]*\).*/\1/p' | tail -1)"
  ffmpeg -v error -y -i "$tmp" -af "volume=$(awk -v p="$peak" 'BEGIN { print -1 - p }')dB" -sample_fmt s16 "$out/$name.wav"
  rm -f "$tmp"
}
# The packs' sounds -> the game's names.
convert "$(find_one punches hit11)" hit_0
convert "$(find_one punches hit12)" hit_1
convert "$(find_one punches hit15)" hit_2
convert "$(find_one punches hit21)" heavy_0
convert "$(find_one punches hit19)" heavy_1
convert "$(find_one punches hit35)" heavy_2
convert "$(find_one breaking bfh1_breaking_01)" crack_0
convert "$(find_one breaking bfh1_wood_breaking_03)" crack_1
convert "$(find_one breaking bfh1_hit_01)" clack_0
convert "$(find_one breaking bfh1_wood_falling_02)" clack_1
convert "$(find_one breaking bfh1_wood_hit_02)" clack_2
convert "$(find_one breaking bfh1_wood_hit_03)" clack_3
convert "$(find_one breaking bfh1_falling_01)" rubble
# The door's warning: knock, knock (two of Kenney's wooden knocks).
tmp="$out/.door.wav"
ffmpeg -v error -y -i "$(find_one kenney impactWood_medium_001)" -i "$(find_one kenney impactWood_medium_003)" -filter_complex \
  "[0]aformat=channel_layouts=mono,atrim=0:0.16,apad=pad_dur=0.06[a];[1]aformat=channel_layouts=mono[b];[a][b]concat=n=2:v=0:a=1" \
  -ac 1 -ar 44100 "$tmp"
convert "$tmp" door
rm -f "$tmp"
echo "imported into $out"
