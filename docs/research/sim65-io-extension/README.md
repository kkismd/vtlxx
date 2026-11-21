# sim65 I/O 拡張の調査証跡

Research: [#593](https://github.com/kkismd/vtlxx/issues/593) · 親Tracker: [#591](https://github.com/kkismd/vtlxx/issues/591) · 比較対象: [#592](https://github.com/kkismd/vtlxx/issues/592)

実験の説明と結論は[#593の最初の報告](https://github.com/kkismd/vtlxx/issues/593#issuecomment-7064831799)、追加検証は[追加報告](https://github.com/kkismd/vtlxx/issues/593#issuecomment-7070241615)、レビューは[レビューコメント](https://github.com/kkismd/vtlxx/issues/593#issuecomment-7070299762)を参照してください。本ディレクトリは後日検証できるよう、scratchから再現に必要なパッチ・小さなprobe・代表ログを保存します。

## 結論

Linux上でsim65のTry専用host hook案を実6502 guestとVTLXX native imageで実証しました。sourceとWaitはcc65の既存blocking `getchar` を保ち、Tryは `JSR $FFF0` で1 byteを試行し、X=0 Byte（Aにraw byte）、1 NotReady、2 EOF、3 Fatalを返す構成です。Waitの約0.75秒入力待ちではCPU時間表示0.00/0.00秒、487,280 cyclesでした。Try EIO、能力検出、stock sim65拒否、loader衝突、別reader競合も記録しています。

これは**実験用コードであり、本番採用された仕様・機能ではありません**。source長・本文へのEIO注入ではruntime開始前に、runtime WaitへのEIO注入ではruntime実行中に、それぞれnative guest status 6で停止することを確認しました。別runtime fixtureでstack/depth/EOF latch保持とbyte未消費を確認しました。source phaseもstatus 6へ停止しますが、frontend固有のエラー分類・診断は実装されていません。Windowsと一般fdの競合reader安全性も未検証です。詳細なPass/Fail、#592との比較、費用見積りは報告コメントを正本とします。

## 環境・識別子

| 項目 | 値 |
|---|---|
| VTLXX | `246d45c9a6ac85a9f8e4e9f762317db35a88e702` |
| cc65 V2.18 | tag `V2.18`, `6efb71bea773837c6fc4aae0c8270b2d70e3e69f` |
| cc65 master | `eebf54af545f4ae52f8489a2f6d4d5f93e7942d2` |
| OS | Ubuntu on WSL2, Linux `6.18.33.2-microsoft-standard-WSL2`, x86_64 |
| CPU / Rust | AMD Ryzen 5 5600T / rustc, cargo `1.99.1` |
| cc65 tools | V2.18 ca65/ld65/sim65、およびmaster sim65。`results/hashes.txt` に一部SHA256 |

VTLXX sourceは当該SHAからscratchへexportしました。cc65ソース全体・実行ファイル・object・imageは含みません。V2.18 tag付属 `cfg/sim6502.cfg` はZPが27 bytesのためnative linkに不足し、実験ではUbuntu package cfg（ZP=256）をscratch cfgへコピーしました。`sim6502.lib` はV2.18 buildの未変更品（SHA256は `results/hashes.txt`）です。

## 再現

必要なものはGit、C build環境（make/compiler）、Python 3、POSIX/Linux、`patch`、Rust toolchain、およびUbuntu cc65 packageの `cfg/sim6502.cfg` です。Windowsでは以下のPOSIX pipe/poll実験を同条件で再現できません。

`cc65-*-try-hook.patch` と `*-capability.patch` はそれぞれ同じbase SHAから適用する独立patchです。後者にはcapability応答も含まれます。V2.18 patchのみ `SIM65_WAIT_FAIL_AFTER` と `SIM65_TRY_FAULT` の故障注入を含みます。master patchに `SIM65_TRY_FAULT` はありません。loader予約差分は `*-reserve-fff0.patch` で、main.cだけに適用する追加差分です。すべてscratch専用です。

以下をrepository rootから実行するとcc65を取得・固定・patch・buildし、VTLXX sourceを固定SHAからexportして、native imageとFatal fixtureを作成し、capability/native/Fatalの代表試験を行います。作業物はmktempで作る `/tmp/vtlxx-593-replay.*` 配下に限られます。

```bash
set -euo pipefail
REPO_ROOT=$(git rev-parse --show-toplevel)
EVIDENCE="$REPO_ROOT/docs/research/sim65-io-extension"
WORK=$(mktemp -d /tmp/vtlxx-593-replay.XXXXXX)
mkdir -p "$WORK/source" "$WORK/image" "$WORK/fatal" "$WORK/target"

git clone https://github.com/cc65/cc65.git "$WORK/cc65-v218"
git -C "$WORK/cc65-v218" checkout 6efb71bea773837c6fc4aae0c8270b2d70e3e69f
patch -p1 -d "$WORK/cc65-v218" < "$EVIDENCE/patches/v218-capability.patch"
cp /usr/share/cc65/cfg/sim6502.cfg "$WORK/cc65-v218/cfg/sim6502.cfg"
make -C "$WORK/cc65-v218" -j2

git clone https://github.com/cc65/cc65.git "$WORK/cc65-master"
git -C "$WORK/cc65-master" checkout eebf54af545f4ae52f8489a2f6d4d5f93e7942d2
patch -p1 -d "$WORK/cc65-master" < "$EVIDENCE/patches/master-capability.patch"
make -C "$WORK/cc65-master" -j2

git -C "$REPO_ROOT" archive 246d45c9a6ac85a9f8e4e9f762317db35a88e702 | tar -x -C "$WORK/source"
patch -p1 -d "$WORK/source" < "$EVIDENCE/patches/vtlxx-sim65-adapter.patch"
VTLXX_ROOT="$WORK/source/crates/vtlxx"
CC65_ROOT="$WORK/cc65-v218" VTLXX_ROOT="$VTLXX_ROOT" OUT_DIR="$WORK/image" \
  bash "$EVIDENCE/probes/build_vtlxx_image.sh"
CC65_ROOT="$WORK/cc65-v218" VTLXX_ROOT="$VTLXX_ROOT" OUT_DIR="$WORK/fatal" \
  bash "$EVIDENCE/probes/build_wait_fatal_probe.sh"

cd "$WORK"
"$WORK/cc65-v218/bin/cl65" -t sim6502 -o "$WORK/capability.bin" \
  "$EVIDENCE/probes/capability_main.c" "$EVIDENCE/probes/capability_probe.s"
"$WORK/cc65-v218/bin/cl65" -t sim6502 -o "$WORK/byte-probe.bin" \
  "$EVIDENCE/probes/capability_byte_probe.c"
"$WORK/cc65-v218/bin/cl65" -t sim6502 -o "$WORK/try-probe.bin" \
  "$EVIDENCE/probes/probe_main.c" "$EVIDENCE/probes/probe_guest.s"

export SIM65_V218="$WORK/cc65-v218/bin/sim65"
export SIM65_MASTER="$WORK/cc65-master/bin/sim65"
export SIM65_STOCK=/usr/bin/sim65
export SIM65_CAPABILITY_IMAGE="$WORK/capability.bin"
export SIM65_BYTE_PROBE="$WORK/byte-probe.bin"
SIM65_V218="$SIM65_V218" SIM65_MASTER="$SIM65_MASTER" SIM65_STOCK="$SIM65_STOCK" \
  SIM65_CAPABILITY_IMAGE="$SIM65_CAPABILITY_IMAGE" SIM65_BYTE_PROBE="$SIM65_BYTE_PROBE" \
  python3 "$EVIDENCE/probes/capability_cases.py"
SIM65_CAPABILITY_IMAGE="$SIM65_CAPABILITY_IMAGE" VTLXX_IMAGE="$WORK/image/vtlxx.prg" \
  SIM65_V218="$SIM65_V218" SIM65_MASTER="$SIM65_MASTER" SIM65_STOCK="$SIM65_STOCK" \
  python3 "$EVIDENCE/probes/capability_vtlxx_e2e.py"

cargo build --manifest-path "$WORK/source/Cargo.toml" -p vtlxx --bin vtlxx6502 --target-dir "$WORK/target"
export VTLXX6502="$WORK/target/debug/vtlxx6502"
export WAIT_FATAL_SOURCE="$EVIDENCE/probes/wait-fatal.vtl"
export CC65_BIN="$WORK/cc65-v218/bin"
export WAIT_FATAL_IMAGE="$WORK/fatal/wait_fatal.prg"
python3 "$EVIDENCE/probes/wait_fatal_cases.py"
SIM65_BIN="$SIM65_V218" SIM65_IMAGE="$WORK/try-probe.bin" \
  python3 "$EVIDENCE/probes/tty_mode_probe.py"
python3 "$EVIDENCE/probes/pipe_reader_race.py"
```

期待値: capability casesはpatched V2.18/masterがstatus 65、stockとfake wrong/error/timeoutがstatus 125。VTLXX native frameの結果はpatched両版でstdout `6590`, stderr空, status 0、stockはstatus 125でguest未起動。Fatal casesは通常EOFがstdout `-1`, stderr空, status 0、source長/source本文/runtime WaitのEIO注入がstdout空・stderrにstatus 6の停止診断・guest status 6でruntime outputなし。source phaseのstatus 6は停止確認であり、frontend専用error分類を示しません。別runtime fixtureは正常入力でstderr/stdout空・status 99・byte `Z`消費、EIO注入でstdout `K`・stderr空・status 6・`Z`未消費（runtime Cell `$1234`、depth 1、EOF latch 0）。TTYとpipe raceは各logを参照してください。

V2.18 tag付属cfgはZP=27 bytesでlinkできないため、上記はUbuntu packageのZP=256 cfgをscratch cc65 treeへコピーします。固定パッケージcfgがない環境では同じメモリ領域設定のcfgを用意し、出所を記録してください。capability E2Eの入力frameは `[u16 LE length][source bytes][runtime bytes]` です。

loader境界fixtureは別途 `cc65-v218-reserve-fff0.patch` / `cc65-master-reserve-fff0.patch` を各対象treeへ適用したsim65を追加して比較します。`loader_collision_cases.py` は `SIM65_TEMPLATE_IMAGE` と5 simulator pathの環境変数を受け取りfixtureを生成します。TTYは `SIM65_BIN` / `SIM65_IMAGE` を使います。

## 成果物

* `patches/`: cc65 V2.18/masterのTry hook、能力応答、loader予約、およびscratch VTLXX adapter差分。各diffはベースSHAを上記表に固定しています。
* `probes/`: raw byte、能力確認、Wait Fatal、source delay、loader衝突、pipe競合、TTY、VTLXX scripted sourceの小さな入力・スクリプト。
* `results/`: scratch実測ログの抜粋、各種識別子。コンパイラ出力・大規模なbuild log・生成binaryは保存していません。
* `licenses/cc65-LICENSE.txt`: cc65から採取したzlib license本文。

## 互換性と適用範囲

`$FFF0` はcc65が公開予約したABIではありません。V2.18で従来ロード可能な `$FFF0..$FFF3`、masterで新たに予約が必要になる `$FFF0` が影響範囲です。実験で一般imageが `$FFF0` のコードを使うと、旧sim65では通常処理・拡張sim65ではhook実行となり、挙動が変わりました。loader予約を追加すれば当該領域まで伸びる既存imageを拒否します。従って既存VTLXX native imageの代表試験成功を、全sim6502 image互換性の証明とは扱いません。上流提案には正式な予約、loader検査、回帰テスト、文書化が必要です。

Tryの `poll(timeout=0)` → blocking `read(1)` は同一pipeのreaderが一つの場合に即時性を確認しました。既存CLIではhost workerが親stdinを読み、sim65 child pipeへ書き、childのsim65のみがpipeを読みます。別readerがpoll後にbyteを奪うとreadがblockし得るため、適用条件は管理下の単一reader pipeです。pipe全体を `O_NONBLOCK` にするとdup descriptorにも影響しWaitのblocking保証を壊すので採用していません。

## 未検証・ライセンス

PTY canonical/rawの挙動とLinux pipe競合は記録済みですが、全シグナル・全poll異常、Windows実装、他OSは網羅していません。cc65はzlib license（本文は `licenses/cc65-LICENSE.txt`）です。上流ソース由来のpatchを配布する場合も元ソースのnoticeを保持し、変更したファイルであることを明示してください。
