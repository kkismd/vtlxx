# VTLXX（旧称 Extended Classic VTL / ECVTL）

VTLXX は、classic VTL の簡潔な表面構文を発展させ、値スタック、通常のユーザー定義手続き、IfElse / While、Rust参照実装と実際の6502ネイティブコンパイラを備える実験的な言語処理系です。

この crate は、従来 ECVTL と呼んでいた**現在の正式な VTLXX 実装**です（[採用 ADR #540](https://github.com/kkismd/vtlxx/issues/540)）。M01 の独立した bootstrap 計画は廃番です。SourceProcedure と手続テンプレートも、それぞれ ADR #433 / #528 により廃止されています。

Rust package `vtlxx`、path `crates/vtlxx/`、CLI `vtlxx` / `vtlxx6502` は正式名称へ移行済みです。

この README は、**現在の `main` で VTLXX source を読み書きする人間およびエージェント向けの実用入口**です。完全な言語仕様書ではありません。

## Source of Truth

- 重要な設計判断の正本は GitHub の採用済み ADR issue です。
- 現在の実装事実と挙動の正本は `src/` と `tests/` です。
- 現在の開発段階と未実装項目は Tracker issue を正本とします。
- user-facing example は [`examples/eight_queens.vtl`](examples/eight_queens.vtl) です。
- formatting rule は [#200](https://github.com/kkismd/vtlxx/issues/200) で整理しています。

特に、VTLXX は開発中です。採用済み ADR に存在していても、まだ `main` に実装されていない surface があります。この README の「現在使える構文」は `main` のコードとテストに合わせています。

関連する主要な ADR / Tracker:

- [#132](https://github.com/kkismd/vtlxx/issues/132): comment / whitespace / logical line
- [#135](https://github.com/kkismd/vtlxx/issues/135): shared value stack と stack effect
- [#145](https://github.com/kkismd/vtlxx/issues/145): identity × source role binding
- [#150](https://github.com/kkismd/vtlxx/issues/150): expression 上の source role
- [#187](https://github.com/kkismd/vtlxx/issues/187): 手続き / 関数 / 拡張プロパティ / 拡張演算子という user-facing terminology
- [#190](https://github.com/kkismd/vtlxx/issues/190): zero-operand invocation
- [#191](https://github.com/kkismd/vtlxx/issues/191): anonymous block / structured control の設計
- [#265](https://github.com/kkismd/vtlxx/issues/265): anonymous block の `[ ... ]` 一本化
- [#280](https://github.com/kkismd/vtlxx/issues/280): value stack 接続記号を `~` に変更
- [#469](https://github.com/kkismd/vtlxx/issues/469), [#255](https://github.com/kkismd/vtlxx/issues/255): 名前付き整数の構文と定義順
- [#217](https://github.com/kkismd/vtlxx/issues/217): target profile ごとの indexed storage の容量とアドレス規則
- [#433](https://github.com/kkismd/vtlxx/issues/433), [#528](https://github.com/kkismd/vtlxx/issues/528): SourceProcedure・手続テンプレートの廃止
- [#541](https://github.com/kkismd/vtlxx/issues/541): VTLXX への名称移行 Tracker

## 実行方法

repository root から source file を実行します。

```sh
cargo run -p vtlxx --bin vtlxx -- path/to/program.vtl
```

Eight Queens example:

```sh
cargo run -p vtlxx --bin vtlxx -- \
  crates/vtlxx/examples/eight_queens.vtl
```

開発時の基本確認:

```sh
cargo test --manifest-path crates/vtlxx/Cargo.toml
cargo test --workspace
```

Rust reference の `Machine` は `set_input` で実行時入力の `std::io::Read` を受け取ります。`$` の PrimaryRead は NUL を読み捨てて `1..255` または EOF の `-1` を返し、入力エラーは runtime error になります。入力位置は同じ Machine の手続きと top-level で共有されます。未設定時は空入力です。現在のRust対話CLIは、runtime stdinをMachine入力へ接続し、stdoutを逐次出力します（#313）。

## NMOS 6502 コンパイラの手動実行

`vtlxx6502` は VTLXX source を既存の NMOS 6502 compiler で native code にし、`sim65` で実行します。Rust reference VMへのフォールバックはありません。実行には cc65 の `ca65`、`ld65`、`sim65` と `sim6502.lib` が必要です。cc65 を導入した後、repository root から実行してください。

```sh
cargo run -p vtlxx --bin vtlxx6502 -- \
  crates/vtlxx/examples/sim65_cli.vtl
```

source file は raw bytes で読み、最大 65535 bytes までです。target は stdin の先頭に source 長の u16 little-endian 2 byte を受け取り、その後に source bytes と runtime input bytes を読みます。任意の runtime input file は `--runtime-input` で指定できます。

6502 frontend では `$` を待機文字入力として利用できます（#317、採用 ADR #328）。`--runtime-input` で指定した raw bytes は source frame の後ろに渡され、VTLXX の `$` から読み取られます。物理 NUL (`0x00`) は読み捨てられ、`1..255` は値として返り、入力 EOF は `-1` になります。したがって `0xff` は EOF ではなく `255` です。

```sh
cargo run -p vtlxx --bin vtlxx6502 -- \
  path/to/program.vtl --runtime-input path/to/input.bin
```

Unix ホストの端末で対話するには `--interactive` を指定します。prompt は改行がなくても表示され、通常の端末設定では文字の後に Enter を押して入力します。プログラムが用意する終了操作で終了でき、Ctrl-C でも中断できます。

```sh
cargo run -p vtlxx --bin vtlxx6502 -- path/to/program.vtl --interactive
```

1文字の入力を試すには、Rust/Cargo と上記の cc65 ツールを PATH に用意し、repository root から次を実行します。

```sh
cargo run -p vtlxx --bin vtlxx6502 -- \
  crates/vtlxx/examples/interactive_input.vtl --interactive
```

改行のない `ENTER ONE CHARACTER: ` が表示されたら、通常の端末設定で `A` を押して Enter で確定します。入力文字と改行は端末の echo による表示です。

```text
ENTER ONE CHARACTER: A
ASCII: 65
```

サンプルは1文字を読み、その ASCII 値を十進数で表示して正常終了します。追加の入力は不要です。EOF の場合は `ASCII: -1` を表示します。

対話モードの stdin は pipe や file にも接続できます。再現可能な入力を使う一括実行には `--runtime-input path/to/input.bin` を使えます。このオプションと `--interactive` は併用できません。対話モードでは stdout を逐次出力します。

sim65 の実行上限は一括モードでは既定で 4,000,000 cycles、対話モードでは無制限です。どちらのモードでも `--cycle-limit` に正の10進数を指定できます。

```sh
cargo run -p vtlxx --bin vtlxx6502 -- \
  path/to/program.vtl --cycle-limit 256000000
```

たとえば `program.vtl` に `A=$ ?=A`、`input.bin` に byte `0x41` を置くと、stdout に `65` が出ます。source-level の入力経路は次の ignored integration test でも実機相当の `sim65` 実行を通して確認できます。

```sh
cargo test -p vtlxx --test sim65_cli -- --ignored
```

成功時の stdout は sim65 が出した bytes そのものです。CLI の診断は stderr に出し、usage・file・toolchain・assemble/link・compile/runtime・cycle-limit failure は nonzero status で終了します。実行は `ca65 -t sim6502` で target source を組み立て、`ld65 -t sim6502` で `sim6502.lib` と link し、生成物を一時 directory に置いて sim65 へ渡す流れです。成功・失敗のどちらも一時生成物は削除します。

手動統合確認:

```sh
cargo test -p vtlxx --test sim65_cli -- --ignored
```

## 基本モデル

### Cell

実行時の値は signed 16-bit integer (`i16`) です。

`+`, `-`, `*` は 16 bit で wrap します。

```text
32767+1  -> -32768
```

`/` と `%` は signed integer division / remainder です。0 除算は runtime error です。`-32768/-1` は `-32768`、`-32768%-1` は `0` になります。

比較結果は `0` または `1` です。条件判定では `0` が偽、0 以外が真です。

### Register

`A` から `Z` は 26 個の register です。起動時はすべて `0` です。

```vtl
  A=10
  B=A+1
```

### Storage

storage は target profile ごとにアドレス幅が決まる ring 状の記憶領域です（採用 ADR [#217](https://github.com/kkismd/vtlxx/issues/217)）。Rust reference は **16 bit / 65536 Cell**、現行 6502/sim65 は **8 bit / 256 Cell** です。どちらも値そのものは signed 16-bit Cell です。

```vtl
  @=10,42
  A=@(10)
```

`@=address,value` が書き込み、`@(address)` が読み取りです。

address は Cell の raw 16-bit pattern の**下位アドレス幅ぶん**を使います。負の Cell も同じ規則で扱い、容量を超えたアドレスは ring 上で循環します。

```vtl
  @=-1,42
  A=@(-1)
```

この例の `-1` は Rust reference では address `65535`、現行 6502/sim65 では `255` を指します。同様に `256` は Rust では `256`、6502 では `0` を指します。複数のターゲットで同じ結果を必要とするプログラムでは、利用する storage のアドレスを共通範囲に収めてください。

### Value stack

VTLXX の procedure / primitive は共通の value stack を使います。

引数個数や戻り値個数の arity metadata は持ちません。stack effect は programmer contract です。

```text
; ( address value -- )
```

call / return 時に data stack は自動 cleanup されません。callee が残した値は caller からそのまま見えます。

## Source の基本規則

### Statement

基本形は 1 文字の target と `=` です。

```text
target=rhs
```

例:

```vtl
  A=1
  B=A+2
  ?=B
```

### Logical line と複数 statement

physical newline が logical line の終端です。

同じ logical line では、空白または tab が statement separator になります。

```vtl
  A=1 B=2 C=A+B
```

一方、statement 内部に構文上の whitespace は置けません。

```vtl
  A=B+1
```

は有効ですが、次は無効です。

```text
A = B + 1
```

logical line の先頭・末尾 whitespace は無視されます。行頭 indentation は表示上のものだけで、scope や control flow の意味を持ちません。

### Comment

`;` から logical line の終端までが comment です。

```vtl
  A=1 ; initialize A
```

double-quoted string 内の whitespace と `;` は内容として保持されます。

```vtl
  ?="hello; world"
```

現在の string output sugar は ASCII のみで、escape syntax はありません。

## Expression

VTLXX の expression は、小さい parser と source role binding で構成されています。

主な value:

| surface | 意味 |
| --- | --- |
| `123`, `-7` | integer literal |
| `A` .. `Z` | register read |
| `@(expr)` | storage read |
| `(expr)` | grouping |

主な binary operator:

| operator | 意味 |
| --- | --- |
| `+` | add |
| `-` | subtract |
| `*` | multiply |
| `/` | divide |
| `%` | remainder |
| `==`, `!=` | equality |
| `<`, `<=`, `>`, `>=` | comparison |

### Operator precedence

現在の VTLXX では、binary operator ごとの precedence はありません。operator は同じ優先順位で**左結合**です。

```vtl
  A=2+3*4
```

は `(2+3)*4` と評価され、`20` になります。

通常の算術優先順位が必要なら grouping を明示します。

```vtl
  A=2+(3*4)
```

これは `14` です。

単項 minus 演算子はありません。`-7` のような負の integer literal は使えますが、`-A` は使えません。

### 名前付き整数

トップレベルの source sequence で `.NAME := value` と書くと、整数に名前を付けられます。参照するときも `.NAME` と書きます。定義は runtime code を生成しません。

```vtl
.BOARD_BASE := 16
.LOOP := 1
  A=.BOARD_BASE+2
^=.LOOP
  #=.LOOP
```

名前本体は ASCII 大文字で始まる 2〜16 byte の `[A-Z][A-Z0-9_]+` で、接頭の `.` は長さに含めず、定数表にも保存しません。1文字の `A`〜`Z` は従来どおり register です。値は `-32768`〜`32767` の符号付き decimal literal 1個に限ります。定数名や式を値には書けません。旧宣言 `==NAME,value` と、`.` を付けない複数文字の定数参照は受理しません。式の等価比較 `==` は引き続き使えます。

宣言の前後は通常の statement と同じ区切りを使います。例えば `A=1 .MAX:=10 B=.MAX` や `.MAX:=10 .MIN:=-1` のように同じ行に書けます。`.` と名前、`:` と `=` はそれぞれ連続させ、`:=` の前後には ASCII space / HTAB を入れられます。宣言の途中で改行することはできません。値の直後には空白、改行、`;` コメントまたは source の終端が必要です。

定義後の RHS 式、数値ラベルの `^=` と `#=` から参照できます。手続きや anonymous block の中で定義することはできません。未定義名の参照と同名の再定義は compile error です。異なる名前に同じ値を指定できます。`^=` / `#=` で使う値には既存のラベル範囲 `0..32767` が適用されます。

定数表は compile-run の source processing 中だけ存在し、runtime に定数名や表を残しません。Rust の `execute_source` では呼び出し1回ごとに表を作ります。手続きの本体は定義時に解決済みの値を保持しますが、別の `execute_source` 呼び出しで定数名を再利用するには再定義が必要です。6502 の現行 sim65 構成は compile-run あたり最大128件の定数宣言を受け付け、Rust reference はこの件数上限を設けません（採用 ADR [#528](https://github.com/kkismd/vtlxx/issues/528)）。

### Comma operand

comma は operand を左から右へ評価し、値を stack に残します。tuple value は作りません。

```vtl
  @=I,V
```

は概念上 `I`, `V` の順に stack へ積み、`@` の Write binding を呼びます。

## Built-in Write surface

### Register write

```vtl
  A=42
```

RHS を評価し、結果を register `A` へ格納します。

### Storage write

```vtl
  @=10,42
```

stack effect は概念上:

```text
( address value -- )
```

### Decimal output

```vtl
  ?=-12
```

signed decimal text を出力します。

### Character output

```vtl
  $=65
```

Cell の low 8 bits を 1 byte として出力します。この例は `A` を出力します。

### String output sugar

```vtl
  ?="hello world"
```

quoted ASCII bytes を順に character output します。string は一般の runtime value ではありません。
actual 6502 compiler でも同じ構文を使用でき、改行は自動追加されません。

### Newline

```vtl
  ?=()
```

LF (`10`) を 1 byte 出力します。

`?=` は現在は無効です。

## Stack 接続

通常の statement は概念上、

```text
RHS を評価して stack に置く
    ↓
target の Write binding が消費する
```

という形です。`~` はこの途中状態を source から直接使うための surface です。

### `~=expr`: 結果を stack に残す

```vtl
  ~=A+1
```

RHS を評価しますが、Write target へ渡しません。そのため結果は stack に残ります。

comma と組み合わせれば複数値を残せます。

```vtl
  ~=10,20
```

### `target=~`: 既存 stack を target へ渡す

```vtl
  A=~
```

新しい RHS value を生成せず、現在の stack top を `A` の Write semantics へ渡します。

source-defined procedure に対しても同じです。

```vtl
  p=~
```

procedure が何値を消費するかは、その procedure の stack-effect contract に従います。

### RHS root の `~`

RHS の先頭に `~` を置くと、既存 stack top を最初の value として expression を続けられます。

```vtl
  A=~+2
```

既存 stack top が `10` なら、`A` は `12` になります。

この `~` は expression の任意位置で使える一般 value ではありません。RHS root の先頭に限定された structural surface です。

## Numeric label と jump

### Label definition

```vtl
^=10
  A=A+1
```

label は decimal `0..32767` です。同じ executable owner 内で一意でなければなりません。

### Unconditional jump

```vtl
  #=10
```

同じ owner の numeric label へ jump します。forward / backward の両方を使えます。

label は top-level と source-defined procedure で別 namespace です。

### RETURN `#=()`

```vtl
  &=p [
    ?=1
    #=()
    ?=9
  ]
  p=()
  ?=2
```

`#=()` は現在の手続きから呼出元へ直ちに戻ります。IF / While の入れ子の中でも、最も内側の block や loop ではなく手続き全体を終了します。RETURN 自身の stack effect は `( -- )` で、値スタックを変更しません。実行済みの register、storage、output の副作用も保持します。

トップレベルでも使用でき、その場合は現在の top-level chunk だけを終了します。後続の手続き定義などで分割された独立 chunk は、driver が通常どおり呼び出します。これはプログラム全体を終了する命令ではありません。

`#=n` は従来どおり owner-local label への GOTO です。RETURN は label や fixup を使わず、backend の既存 RTS を出力します。通常の手続き / chunk 末尾に自動追加される RTS も維持されます。

## IF / While

### IF `%=`

`%=` は condition に続けて必須の then block と、省略可能な else block を取ります。condition が 0 以外なら then、0 なら else を実行します。else がなければ 0 のときは何も実行しません。

```vtl
  %=A<10 [
    B=B+1
    ?=B
  ]
```

then / else はどちらも `[ ... ]` anonymous block に限ります。単独の ordinary statement は arm にできません。

```vtl
  %=A<10 [
    B=1
  ] [
    B=2
  ]
```

then block の次の source form が `[` なら、その block を else として1つだけ読みます。それ以外の form は IF の後続として残します。改行、空行、comment は else の区切りになりません。else opener の後で block が不正なら IF 全体が compile error です。

```vtl
  %=A [
    B=1
  ]
  ; この comment を挟んでも次の block は else
  [
    B=2
  ]
```

block は入れ子にでき、`[` は `]` で閉じます。`[` / `]` は空白または logical-line separator で独立した token にします。同じ行にも書けますが、複数行形式を推奨します。quoted string や comment 内の delimiter は認識されません。stack 接続には `~` を使います。body argument を要求していない位置に bare delimiter を置くことはできません。`|= ... =|` は anonymous block として使えません。

anonymous block は独立した procedure や runtime value ではありません。surrounding executable owner の code へ inline に構築され、numeric label namespace も surrounding owner と共有します。

statement target の `%=` は IF source command です。expression 中の `%` は従来どおり remainder operator であり、意味は変わりません。

### While `*=condition`

`*=` は IF と同じ通常の RHS 式を条件とし、直後に **1つの `[ ... ]` anonymous block** を body として取ります。条件と block の間には空白または改行が必要です。単独 statement の省略形はありません。

```vtl
  A=0
  *=A<10 [
    A=A+1
  ]
```

条件式は一度コンパイルされ、その生成コードを各反復の先頭で実行します。既存の `JumpIfZero` が条件結果の 1 Cell を消費し、0なら終了、非0なら body を実行して条件を再評価します。括弧、名前付き整数、乗算も通常の RHS と同じ規則で使えます。

While は stack を自動 cleanup しません。式が必要とする stack input と副作用は通常の式規約に従い、開始前からある値や body が残した値は共通の value stack 上に残ります。body は surrounding owner の code と numeric label namespace を共有します。

空条件と zero-operand marker は不正です。旧 predicate block + body の2ブロック形式も compile error になります。statement の `*=` は native While source command、expression 内の `*` は multiplication operator です。

Rust reference と actual 6502 frontend の両方でこの構文を使います。sim65 共通適合検証では、各 While 使用 example の出力と Rust reference の結果を照合します。

## Source-defined 手続き

user-defined Write binding は、利用者向けには「手続き」と呼びます。通常の手続きは `&=name [ ... ]` で定義し、`name=RHS` または `name=()` で呼びます。例えば、次のように複数文字名を使用できます。

```vtl
  &=showval [
    ?=~
  ]

  showval=42
```

通常の手続き名は Rust reference と 6502 の両方で **ASCII 小文字 1〜8 文字**（`[a-z]{1,8}`）です。1文字名も有効で、`a` と `ab`、`read` と `reader` は別名として扱います。空名、大文字、数字、underscore、9文字以上の名前は使用できません。名前の伸長は通常の user-defined Write に限られ、A-Z register、組み込み語、`.NAME` 名前付き整数の構文は変わりません。

**手続きの定義件数は名前長とは別の制限**です。Rust reference は通常手続きに人為的な64件上限を設けず、6502初期targetは通常手続きの定義を最大64件まで受け付けます。ただしarena等の別の資源上限により、64件の定義が常に成功する保証はありません。65件目の定義ではコンパイルに失敗し、6502では生成された runtime を実行しません。テンプレート用の名前空間は採用ADR [#528](https://github.com/kkismd/vtlxx/issues/528) により廃止されています。

廃止された手続テンプレート構文（例: `&=a{} [ {}=A ]`、`a{A}=()`、`a{A}=1`）は、Rust・6502のどちらでも**コンパイル時の構文エラー**になります。通常Writeの定義と呼出は引き続き利用できます。拒否の回帰は [`tests/source_execution.rs`](tests/source_execution.rs)、[`tests/sim65_frontend.rs`](tests/sim65_frontend.rs)、[`tests/sim65_cli.rs`](tests/sim65_cli.rs) で確認します。

例:

```vtl
  ; ( address value -- )
  &=p [
    V=~
    I=~
    @=I,V
  ]

  p=10,42
```

procedure は caller と同じ value stack を使います。`p=10,42` は `10`, `42` を左から右へ stack に積んでから `p` を呼びます。

### Zero-operand invocation

operand を追加せず procedure を呼ぶには `()` を使います。

```vtl
  &=q [
    ?="called"
  ]

  q=()
```

`()` は UNIT value ではありません。「通常 operand を 0 個生成する」という source marker です。

callee が stack input を要求する場合、zero-operand call でも existing caller stack はそのまま見えます。空 stack で必要値が足りなければ通常の stack-underflow error になります。

### Definition order

source から procedure を公開するときは、本文のコンパイルと完成が成功してから binding に登録します。call site は source-processing 時に完全名を解決し、解決済みの呼出先を使用します。runtime で名前を再検索する仕組みはありません。同一完全名の再定義、未定義呼出、呼出先が後に定義される forward call はコンパイルエラーです。

依存する procedure は、callee を先に定義してください。コンパイル失敗で未完成の手続きは公開されず、既に完成した手続きの binding は維持されます。

### 長い通常手続名の現行サンプル

手続き数が多い例では、役割が分かる名前を使っています。[`micro_trek.vtl`](examples/micro_trek.vtl) は `random`、`longscan`、`gameend` など23手続、[`1024.vtl`](examples/1024.vtl) は `newtile`、`movegrid`、`gameloop` など9手続を定義しています。[`eight_queens.vtl`](examples/eight_queens.vtl) と [`eight_queens_storage.vtl`](examples/eight_queens_storage.vtl) は `collide`、`printsol`、`search` など、[`maze_dfs.vtl`](examples/maze_dfs.vtl) は `visit`、`pushpath`、`showlen` などを使います。

小規模な [`rpn_evaluator.vtl`](examples/rpn_evaluator.vtl) のように、短い1文字の通常手続名を残すことも有効です。通常Writeの長名・失敗時の意味論は [`tests/long_write_names.rs`](tests/long_write_names.rs)、6502の件数上限と複数chunkの実行順序は [`tests/sim65_long_write_names.rs`](tests/sim65_long_write_names.rs) で検証しています。

## Source role の用語

VTLXX の binding model は identity と source role の組です。

利用者向けには次の用語を使います。

| internal role | user-facing terminology |
| --- | --- |
| Write | 手続き |
| AppliedRead | 関数 |
| PrimaryRead | 拡張プロパティ |
| BinaryOperator | 拡張演算子 |

ただし current `main` で source から定義できる user-defined role は Write、つまり**手続きだけ**です。

関数・拡張プロパティ・拡張演算子は意味モデルと用語が決まっていますが、未実装の definition surface を推測して source に書かないでください。

## Formatting / indentation

indentation は構文ではありませんが、user-facing VTLXX source では [#200](https://github.com/kkismd/vtlxx/issues/200) の規則を使います。

### 基本

- indentation は ASCII space のみ
- tab は使わない
- 1 level は 2 spaces
- blank line に不要な whitespace を残さない
- indentation depth は owner / block nesting から一意に決め、任意にずらさない

current `main` の named procedure と numeric label では:

```text
column 0  top-level numeric label
column 2  top-level statement
          procedure start / end
          procedure-local numeric label
column 4  procedure body statement
```

例:

```vtl
  &=q [
    A=~
  ^=1
    ?=A
    A=A-1
    %=A [ #=1 ]
  ]

  A=3
  q=A
```

top-level label:

```vtl
  A=3
^=1
  ?=A
  A=A-1
  %=A [ #=1 ]
```

### Comment indentation

独立 comment は、説明対象の code と同じ indentation depth に置きます。

```vtl
^=1
  ; decrement A until zero
  A=A-1
  %=A [ #=1 ]
```

procedure body の説明なら body と同じ位置です。

```vtl
  &=q [
    ; consume the top value
    A=~
  ]
```

trailing comment は code との間に 1 文字以上の ASCII space を置きます。

```vtl
  A=A-1    ; decrement
  %=A [ #=1 ]  ; continue while nonzero
```

必要なら comment の `;` を局所的に揃えてかまいませんが、固定 comment column は設けません。

### Anonymous block の indentation

anonymous block では owner depth に block nesting depth を加えます。block に入るごとに 1 level、つまり 2 spaces 増やします。

```vtl
  %=A [
    B=1
    %=B [
      C=2
    ]
  ]
```

block 内の numeric label は意味論上 surrounding owner の label namespace を共有しますが、表示上は block nesting depth の基準位置へ置きます。

```vtl
  %=A [
  ^=1
    B=B+1
    %=B<10 [ #=1 ]
  ]
```

opener / closer の改行配置には今後評価する余地がありますが、同一 source 内では style を統一し、indentation depth 自体は #200 の rule から一意に決めます。

## Example: 1024

[`examples/1024.vtl`](examples/1024.vtl) は 4×4 の盤面で同じ数を合わせて 1024 を目指すゲームです。repository root から Rust CLI で起動します。

```sh
cargo run -p vtlxx --bin vtlxx -- \
  crates/vtlxx/examples/1024.vtl
```

`W` / `A` / `S` / `D` で上下左右、`R` で新しいゲーム、`Q` で終了します。小文字も使えます。通常の端末ではコマンドを入力して Enter を押してください。入力文字と改行は端末の echo として表示されます。

同じ source は 6502/sim65 でも実行できます。固定入力による終了は、[`tests/sim65_examples.rs`](tests/sim65_examples.rs) の ignored 統合テストで、Rust reference と 6502 の正常終了・stdout 一致を検証しています。検証条件は `1024_quit.txt` を入力に使い、cycle 上限を `256000000` とした一括実行です。

```sh
cargo run -p vtlxx --bin vtlxx6502 -- \
  crates/vtlxx/examples/1024.vtl \
  --runtime-input crates/vtlxx/tests/fixtures/1024_quit.txt \
  --cycle-limit 256000000
```

端末から操作する場合は、次の対話モードを使用します。ただし、上記の固定入力テストは対話操作の全経路まで検証するものではありません。

```sh
cargo run -p vtlxx --bin vtlxx6502 -- \
  crates/vtlxx/examples/1024.vtl --interactive
```

## Example: Eight Queens

[`examples/eight_queens.vtl`](examples/eight_queens.vtl) は current VTLXX の user-facing example です。

次をまとめて使っています。

- A-Z register
- storage
- source-defined procedures for board writes, collision checks, output, and search
- shared value stack
- IF `%=` and While `*=condition` with anonymous blocks
- decimal / newline output
- formatting / indentation rule

手続きは callee-first で定義し、stack effect と共有 register / storage の読み書きを source comment に記載しています。探索・衝突判定・解の表示では anonymous block と structured control を使い、numeric label / jump は使っていません。

実行:

```sh
cargo run -p vtlxx --bin vtlxx -- \
  crates/vtlxx/examples/eight_queens.vtl
```

92 個の解を列挙し、最後に解数 `92` を出力します。

[`examples/eight_queens_storage.vtl`](examples/eight_queens_storage.vtl) は、列ごとの候補と解数を storage に置き、現在の候補だけを短時間 register に読み出す別例です。同じ92解を逐一表示します。

## Example: 固定迷路の DFS

[`examples/maze_dfs.vtl`](examples/maze_dfs.vtl) は、固定 4×4 迷路を再帰を使わずに探索します。迷路、訪問済み状態、現在の経路、各深さで次に試す方向を別々の indexed storage 領域に置きます。右から進んだ cell 2 の行き止まりで一度戻り、goal への経路の cell 数 `7` を表示します。

この例では `.VISITED_BASE`、`.PATH_BASE`、`.DIRECTION_BASE` を定義し、storage 領域の基点を手続き内から参照します。

```sh
cargo run -p vtlxx --bin vtlxx -- \
  crates/vtlxx/examples/maze_dfs.vtl
```

## Example: 固定 token 列の RPN evaluator

[`examples/rpn_evaluator.vtl`](examples/rpn_evaluator.vtl) は source 自身が indexed storage に `3 4 ADD 2 MUL 7 SUB` を初期化し、共有 value stack を operand stack として評価して `7` を表示します。operator 手続きはそれぞれ `( lhs rhs -- result )` で、token は program counter で順に読みます。

```sh
cargo run -p vtlxx --bin vtlxx -- \
  crates/vtlxx/examples/rpn_evaluator.vtl
```

## VTLXX source を書くときの確認事項

新しい VTLXX source を追加・変更するときは、少なくとも次を確認してください。

1. current Tracker で、その surface が `main` に実装済みか確認する。
2. statement 内へ whitespace を入れない。
3. binary operator の通常の precedence を仮定せず、必要なら `(...)` を使う。
4. procedure の stack effect を comment で明示する。
5. procedure 呼び出しは callee-first の定義順を守る。
6. numeric label は executable owner ごとの local namespace として扱う。
7. indentation は 2 ASCII spaces 単位で #200 の owner rule に従う。
8. 未実装の関数・拡張プロパティ・拡張演算子を推測で書かない。
9. current code / tests / accepted ADR と README が食い違う場合は、README を正本として扱わず差分を確認する。
