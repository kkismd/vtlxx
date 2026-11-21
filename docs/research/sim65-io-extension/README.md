# sim65 I/O 拡張の調査証跡

Research: [#593](https://github.com/kkismd/vtlxx/issues/593) · 親Tracker: [#591](https://github.com/kkismd/vtlxx/issues/591) · 比較対象: [#592](https://github.com/kkismd/vtlxx/issues/592)

実験の説明と結論は[#593の最初の報告](https://github.com/kkismd/vtlxx/issues/593#issuecomment-7064831799)、追加検証は[追加報告](https://github.com/kkismd/vtlxx/issues/593#issuecomment-7070241615)、レビューは[レビューコメント](https://github.com/kkismd/vtlxx/issues/593#issuecomment-7070299762)を参照してください。本ディレクトリは後日検証できるよう、scratchから再現に必要なパッチ・小さなprobe・代表ログを保存します。

## 結論

Linux上でsim65のTry専用host hook案を実6502 guestとVTLXX native imageで実証しました。sourceとWaitはcc65の既存blocking `getchar` を保ち、Tryは `JSR $FFF0` で1 byteを試行し、X=0 Byte（Aにraw byte）、1 NotReady、2 EOF、3 Fatalを返す構成です。Waitの約0.75秒入力待ちではCPU時間表示0.00/0.00秒、4,872,280 cyclesでした。Try EIO、能力検出、stock sim65拒否、loader衝突、別reader競合も記録しています。

これは**実験用コードであり、本番採用された仕様・機能ではありません**。待機/source read errorからruntime Fatalへ伝えるnative統合、Windows、一般fdの競合reader安全性などは未成立または対象外です。詳細なPass/Fail、#592との比較、費用見積りは報告コメントを正本とします。

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

必要なものはGit、C build環境（make/compiler）、Python 3、POSIX/Linux、Rust toolchain、およびこのrepositoryです。Windowsでは以下のPOSIX pipe/poll実験を同条件で再現できません。

```sh
git clone https://github.com/cc65/cc65.git /tmp/cc65-v218
git -C /tmp/cc65-v218 checkout 6efb71bea773837c6fc4aae0c8270b2d70e3e69f
git -C /tmp/cc65-v218 apply "$PWD/docs/research/sim65-io-extension/patches/cc65-v218-try-hook.patch"
make -C /tmp/cc65-v218 -j2

git clone https://github.com/cc65/cc65.git /tmp/cc65-master
git -C /tmp/cc65-master checkout eebf54af545f4ae52f8489a2f6d4d5f93e7942d2
git -C /tmp/cc65-master apply "$PWD/docs/research/sim65-io-extension/patches/cc65-master-try-hook.patch"
make -C /tmp/cc65-master -j2
```

`cc65-*-try-hook.patch` はTry hookと故障注入を含む実験patchです（V2.18側の `SIM65_WAIT_FAIL_AFTER`、両版の `SIM65_TRY_FAULT`）。`*-capability.patch` はcapability応答も含む**独立した全体patch**で、同じベースSHAから適用します。両方を重ねないでください。loader予約差分は `*-reserve-fff0.patch` で、main.cだけに適用する追加差分です。VTLXX adapter差分は `vtlxx-sim65-adapter.patch` です。これらを製品へ適用しないでください。

guest probeの構築例（各cc65 checkout内）:

```sh
cd docs/research/sim65-io-extension/probes
cl65 -t sim6502 -o /tmp/capability.bin capability_main.c capability_probe.s
cl65 -t sim6502 -o /tmp/byte-probe.bin capability_byte_probe.c
```

当該VTLXX revisionを展開してnative CLI imageを作る場合は、cc65とVTLXXのscratch rootを指定します。`experimental_adapter.s` は調査用adapterです。

```sh
git -C /path/to/vtlxx archive 246d45c9a6ac85a9f8e4e9f762317db35a88e702 crates/vtlxx | tar -x -C /tmp/vtlxx-src
CC65_ROOT=/tmp/cc65-v218 VTLXX_ROOT=/tmp/vtlxx-src/crates/vtlxx \
OUT_DIR=/tmp/vtlxx-image sh docs/research/sim65-io-extension/probes/build_vtlxx_image.sh
```

V2.18 tagのZP領域設定ではlinkが不足します。実験と同じ条件にする場合は、Ubuntu cc65 package由来の `cfg/sim6502.cfg`（ZP=256）を `CC65_ROOT/cfg/sim6502.cfg` にscratch内で適用してください。native CLIのsource frameは2-byte little endian length、続けてsource、runtime bytesです。

能力検出ケースを走らせるには、能力patchを重ねたV2.18/master sim65、stock sim65、fake simulatorを指定します。

```sh
SIM65_V218=/tmp/cc65-v218/bin/sim65 SIM65_MASTER=/tmp/cc65-master/bin/sim65 \
SIM65_STOCK=/usr/bin/sim65 SIM65_CAPABILITY_IMAGE=/tmp/capability.bin \
SIM65_BYTE_PROBE=/tmp/byte-probe.bin python3 probes/capability_cases.py
```

期待値: patched V2.18/masterはbyte probeのstatus 65、stockはstatus 125でstderrに `capability rejected: probe exit 127`、誤応答・異常status・timeoutもstatus 125で拒否します。`capability_vtlxx_e2e.py` はVTLXX native imageを `VTLXX_IMAGE` に指定し、入力frame `[u16 LE length][source][runtime]` `A=` B=$ ?=A ?=B` + `AZ` を渡します。patched simulatorの期待stdoutは `6590`、status 0、stockは本体起動前にstatus 125です。

Fatal/loader/race/TTYのprobeは `probes/` にあります。Fatal試験にはnative build/imageが必要で、`VTLXX6502`, `WAIT_FATAL_SOURCE`, `CC65_BIN`, `SIM65_V218`, `WAIT_FATAL_IMAGE` を設定します。loader fixture生成には `SIM65_TEMPLATE_IMAGE` と `SIM65_STOCK`, `SIM65_V218`, `SIM65_MASTER`, `SIM65_V218_RESERVED`, `SIM65_MASTER_RESERVED` を設定します。生成imageは `probes/` に作られます。競合実験は `python3 probes/pipe_reader_race.py`、TTYは `SIM65_BIN` と `SIM65_IMAGE` を設定して実行してください。

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
