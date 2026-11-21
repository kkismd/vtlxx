# 実験結果の要約

完全な説明、制約、費用比較は[#593の報告](https://github.com/kkismd/vtlxx/issues/593#issuecomment-7070241615)を参照。ログはscratchから採取し、値を加工していない。

| 実験 | 結果 |
|---|---|
| V2.18 / master capability probe | patched両版が応答。stock sim65は `Illegal opcode $FF at address $FFF0` / status 127。launcherは起動前に拒否してstatus 125。fake wrong response、異常status、timeoutも拒否。|
| capability経由native VTLXX | patched両版: status 0, stdout `6590`; stock: status 125, stdout空。|
| Wait/source EIO | 正常EOF: status 0, stdout `-1`。source frame各位置とruntime Waitで注入: status 6、stdout空。別runtime fixtureはstack `$1234` / depth 1 / EOF latch 0を保ち、残り入力 `Z` を未消費（`K` marker、status 6）。 |
| Wait Fatal統合範囲 | source-length/source-payload/runtimeを分離してEIOを注入し、各native実行がguest status 6で停止、runtime outputなし。source phaseのfrontend固有分類は未実装。別runtime fixtureでCell/depth/EOF latch保持とbyte未消費を確認。 |
| loader境界 | stockと無予約のV2.18はend `$FFF0` fixtureを実行(status 42)。patched hookでは `$FFF0` callがhookとなりstatus 66。予約loaderはそのimageを拒否（V2.18変更版status 0、master変更版status 255）。|
| poll/read競合 | poll後に競合readerが `A` を消費すると被験readは100ms超blockし、次の `B` で解除。dup fdの `O_NONBLOCK` は元fdにも反映。|
| TTY | canonical: newline前はNotReady、newline後に`A`、次にLF。raw: `B`を即時取得。|
| 0.75秒Wait | 3回のWait、約250ms間隔。simulator wall 0.72s、CLI wall 0.753s、user/sys表示0.00/0.00s、487,280 cycles、stdout `65/66/67 + cycles`、status 0。CPU時間は0.01秒単位表示の丸め。|

詳細なケース表と#589との条件差はIssueコメントにあります。VTLXX固有のNUL変換・EOF stickyはhost hookの生byte応答と区別して評価しています。`results/hashes.txt` はsim6502.lib/sim65一部のSHA256です。
