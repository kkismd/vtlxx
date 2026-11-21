# mos6502 ホストの調査証跡

[#596](https://github.com/kkismd/vtlxx/issues/596) · Research [#592](https://github.com/kkismd/vtlxx/issues/592) · 親Tracker [#591](https://github.com/kkismd/vtlxx/issues/591) · 方式提案 [#595](https://github.com/kkismd/vtlxx/issues/595)（提案中） · 比較 [#593](https://github.com/kkismd/vtlxx/issues/593)

無改造mos6502の公開APIで、cc65が生成した現行sim6502 imageを実行する実験の証跡です。本番用のホストや採用済みABIではありません。初回・追加の観測と制約は [最初の報告](https://github.com/kkismd/vtlxx/issues/592#issuecomment-7063281210)、[native実行とTry](https://github.com/kkismd/vtlxx/issues/592#issuecomment-7064186776)、[Fatalとoverflow](https://github.com/kkismd/vtlxx/issues/592#issuecomment-7064252154)、[Wait overflowとOS error](https://github.com/kkismd/vtlxx/issues/592#issuecomment-7064258060)、[通常readのerrorとcycle差](https://github.com/kkismd/vtlxx/issues/592#issuecomment-7064415579)を参照してください。

## 保存内容と出所

| 内容 | 出所と用途 |
| --- | --- |
| `probes/harness/` | #592で使ったscratchの旧Rust hostとCargo.lockの原本。read/write/exit、仮入力trapを扱う |
| `probes/failure-harness/` | 後の故障注入とstdout flushを加えたscratch host原本。read失敗の直接停止またはAX=$FFFFを選べる |
| `probes/guest/adapter.s`, `adapter-fatal.s` | scratchの仮guest adapter原本。前者はFatalで直接exit、後者は`rt_input_failure`へ飛ぶ |
| `probes/guest/fatal*.s`, `*capacity.s` | 既存`crates/vtlxx/tests/fixtures/sim65/runtime_try_fatal.s`と`runtime_try_capacity.s`からscratchで作成したfixture。script adapterのcall countを除き、capacityでは後で`serial_in`して未消費byteを確認。wait版は対象helperをWaitへ変更 |
| `probes/check.py`, `timing.py`, `compare.py`, `poll.c`, `vr.c` | 当時のscratch原本。Python 3本は`/tmp/vtlxx-592-exp`固定のため、可搬な再実行には下記runnerを使う。`vr.c`は比較時のC host |
| `reproduce.sh`, `verify.py` | #596で**新たに再構成**したbuild手順と代表caseの自動確認。実験当時の原本ではない |
| `results/replay-linux.txt` | #596で新しいscratchから生成したimageとrunnerを使った再実行ログ。時間は単回測定 |

CPUコア、生成object/image、Cargo target、外部ソースツリーは保存しません。入力trap `$FFEF/$FFF0` は今回の仮接続であり正式なABIではありません。仮adapterを別リンクしたimageと、現行adapterのままのimageは別物です。

## 固定版とライセンス

| 項目 | 実験条件 |
| --- | --- |
| VTLXX source | `246d45c9a6ac85a9f8e4e9f762317db35a88e702`、本repositoryから`git archive` |
| mos6502 | [mre/mos6502](https://github.com/mre/mos6502/tree/9330a8e773702d8b8d92734324027fa916490158)、`9330a8e773702d8b8d92734324027fa916490158`、crate 0.10.1、[BSD-3-Clause](https://github.com/mre/mos6502/blob/9330a8e773702d8b8d92734324027fa916490158/LICENSE) |
| 依存 | Cargo.lockのbitflags 2.13.2、log 0.4.35。CPUソースはcloneして固定、vendorしない |
| vrEmu6502 | 比較記録のみ。[visrealm/vrEmu6502](https://github.com/visrealm/vrEmu6502/tree/aae98cb14386d832cb7357c99626520b6590bc24)、`aae98cb14386d832cb7357c99626520b6590bc24`、[MIT](https://github.com/visrealm/vrEmu6502/blob/aae98cb14386d832cb7357c99626520b6590bc24/LICENSE)。本runnerは取得・実行しない |
| cc65 | ca65/ld65/sim65 V2.18（Ubuntu package 2.19-1）。利用した`sim6502.lib` SHA-256 `37b33d574953ee4ead54e97505f739718c84e6e72c31aed183a18e378c9e0300`。cc65の配布・licenseは[原本](https://github.com/cc65/cc65/blob/master/LICENSE)を参照 |
| 実行環境 | Linux `6.18.33.2-microsoft-standard-WSL2` x86_64、AMD Ryzen 5 5600T、rustc/cargo 1.99.1 |

現行imageのSHA-256は `d22660174100f13f3d2ed1e34a88f89e679f5d2cd2fc623473318392565c297c`、仮adapter imageは `1e3c433decff8c6640e1009deb4a2573842482a3fdd19481f0bdc0a6aff87361`。cc65のパッケージや設定が異なればimageとcyclesも変わり得ます。

## 再現手順

Linux、Git、bash、Python 3、C compiler、ca65/ld65/sim65と`sim6502.lib`、Rust/Cargo（edition 2024対応）、GitHubとCargo registryへの取得手段が必要です。repositoryに上記VTLXX commitがあることを確認し、repository rootから、**まだ存在しない絶対パス**を指定します。生成物・外部依存はそのscratchに残ります。

```sh
bash docs/research/mos6502-host/reproduce.sh /tmp/vtlxx-596-replay-NEW
```

scriptは固定mos6502をclone/checkoutし、Cargo.lockに従って両hostをbuild、同じ現行imageをstock sim65とmos6502へ渡します。別adapter imageとprobeも作り、`verify.py`がstdout・status・代表cyclesをassertします。最後の`PASS: representative cases`が成功です。明示assertのないCPU時間やpollの出力は測定値であり、wall timeやCPU時間を固定値として合否判定しません。既存scratchに対する再実行は次のとおりです。

```sh
python3 docs/research/mos6502-host/verify.py /tmp/vtlxx-596-replay-NEW
```

比較対象のvr実験を再構築する場合は、scratchへ上記SHAをcheckoutし、`cc -O2 -I "$WORK/vrEmu6502/src" docs/research/mos6502-host/probes/vr.c "$WORK/vrEmu6502/src/vrEmu6502.c" -o "$WORK/vr"`、続いて `printf '\004\000?=65' | "$WORK/vr" "$WORK/image/vtlxx.prg"`。これは今回の自動runnerには含めず、#596での再実行は**未実施**です。

## 期待結果と制約

| 確認 | 期待結果 |
| --- | --- |
| 同じ現行image `?=65` | 両hostともstdout `65`、status 0。sim65 `-c`はstdoutに`65466196 cycles`を連結、mosはstderrに`cycles=466289` |
| 現行image | source途中切断status 129、構文失敗status 130、WaitのNUL読み捨て後に`65`。現行adapterのTryはstatus 6 |
| 仮adapter image | 未入力Try `0`、NUL `0`、0xff `255`、EOF二回 `-1-1`、Try/Wait/TryにABCで `656667` |
| Fatal fixture | `K` / status 6。probeは既存Cell・depth・EOF latchを確認する |
| overflow fixture | `K` / status 2。全16 Cellとdepthを検査し、入力byteが残っていることを後から確認する |
| host直接停止 | read status 120、write status 121は**実験用host status**。guestのruntime status 6とは異なる |
| `/dev/full` | 旧hostはbufferされたstdoutをflushせずerrorを取りこぼしstatus 0。failure hostの明示flushはstatus 121 |
| cycle limit | sim65は466196で126・466197で0、mosは466289で126・466290で0。busy Tryはlimitで126 |

750ms Waitと途中遅延sourceはpipeを使い自動測定します。保存ログのWaitはsim65 user 0.0015秒、mos user 0.0024秒、仮adapter mos user 0.0023秒でした。測定は単回であり、環境や負荷で変動します。poll probeには当時の簡易Cコードとして戻り値未確認のコンパイラ警告があります。

旧hostのTryはpoll後にblocking readするため競合時にブロックし得ます。EINTR/EAGAINとfd状態、stdoutのflush、source/WaitのFatal分類を本番用に修正したものではありません。仮source Fatalはruntimeのerror entryへ飛ぶ実験で、責務やABIを確定しません。

cycle差93のうち3はexit命令の計上範囲、残る90の原因は未特定です。完全なcycle一致は確認していません。vrの追加80は実験用trampolineの8回×10 cyclesで、flags変更と予約メモリも伴います。全sim6502 service、全VTLXX workload、非Unix、Windows、端末設定、CLI統合やCIは実証していません。詳細は上記#592の報告を正本とします。
