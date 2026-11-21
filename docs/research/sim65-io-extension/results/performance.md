# 性能記録

計測は#593追加報告と同一値。user/sysは `/usr/bin/time` の0.01秒分解能表示。

| workload | wall | user/sys | sim65 cycles | stdout / 終了 |
|---|---:|---:|---:|---|
| source frame受信後、source payloadを250ms遅延 | simulator 0.24s、CLI 0.305s | 0.00/0.00s | 469,824 | `40` + cycles、status 0 |
| 3回Wait、byteを約250ms間隔（合計0.75s） | simulator 0.72s、CLI 0.753s | 0.00/0.00s | 487,280 | `65/66/67` + cycles、status 0 |
| 1回Wait、750ms遅延、cycle limit 4,000,000 | simulator 0.72s | 0.00/0.00s | 469,906 | `88` + cycles、status 0 |
| pipe open/未入力のTry有限loop 1,000回 | simulator <0.01s、CLI 0.031s | 0.00/0.00s | 1,155,424 | `0` + cycles、status 0 |
| Try loop、cycle limit 500,000 | simulator <0.01s、CLI 0.031s | 0.00/0.00s | limit到達 | stdout空、CLI cycle-limit error |
| Wait中に外部process groupへTERM | 0.252s | 0.00s未満の表示 | guest上限と独立 | status -15 |

Wait/sourceのblocking read中はguestが実行されないためcycle limitはhost wall timeoutになりません。外部wall timeoutまたはキャンセルが別途必要です。Tryをguest program自身が高速反復すればそのguest CPUを使います。

#589の基準は約0.75秒待機、約0.6秒user CPU、約2億cycles、約45万readです。今回の環境・source・workloadは一致していないので厳密な性能比ではなく、blocking Waitでbusy pollingしないことの確認です。
