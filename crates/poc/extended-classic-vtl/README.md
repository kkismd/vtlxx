# Extended Classic VTL (ECVTL) PoC

Extended Classic VTL (ECVTL) は、classic VTL の小さい表面構文を保ちながら、value stack、source-defined procedure、自己拡張可能な source role などを検証するための PoC です。

この crate は VTLXX の最終言語仕様そのものではなく、意味論と bootstrap 構造を検証する reference implementation / executable specification です。

この README は、**現在の `main` で ECVTL source を読み書きする人間およびエージェント向けの実用入口**です。完全な言語仕様書ではありません。

## Source of Truth

- 重要な設計判断の正本は GitHub の採用済み ADR issue です。
- 現在の実装事実と挙動の正本は `src/` と `tests/` です。
- 現在の開発段階と未実装項目は Tracker issue を正本とします。
- user-facing example は [`examples/eight_queens.vtl`](examples/eight_queens.vtl) です。
- formatting rule は [#200](https://github.com/kkismd/vtlxx/issues/200) で整理しています。

特に、ECVTL は開発中です。採用済み ADR に存在していても、まだ `main` に実装されていない surface があります。この README の「現在使える構文」は `main` のコードとテストに合わせています。

関連する主要な ADR / Tracker:

- [#132](https://github.com/kkismd/vtlxx/issues/132): comment / whitespace / logical line
- [#135](https://github.com/kkismd/vtlxx/issues/135): shared value stack と stack effect
- [#145](https://github.com/kkismd/vtlxx/issues/145): identity × source role binding
- [#150](https://github.com/kkismd/vtlxx/issues/150): expression 上の source role
- [#187](https://github.com/kkismd/vtlxx/issues/187): 手続き / 関数 / 拡張プロパティ / 拡張演算子という user-facing terminology
- [#190](https://github.com/kkismd/vtlxx/issues/190): zero-operand invocation
- [#191](https://github.com/kkismd/vtlxx/issues/191): anonymous block / structured control の設計
- [#265](https://github.com/kkismd/vtlxx/issues/265): anonymous block の `[ ... ]` 一本化
- [#194](https://github.com/kkismd/vtlxx/issues/194): EC03 Tracker

## 実行方法

repository root から source file を実行します。

```sh
cargo run -p vtlxx-poc-extended-classic-vtl --bin ecvtl -- path/to/program.vtl
```

Eight Queens example:

```sh
cargo run -p vtlxx-poc-extended-classic-vtl --bin ecvtl -- \
  crates/poc/extended-classic-vtl/examples/eight_queens.vtl
```

開発時の基本確認:

```sh
cargo test --manifest-path crates/poc/extended-classic-vtl/Cargo.toml
cargo test --workspace
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

storage は 65536 Cell の address space です。

```vtl
  @=10,42
  A=@(10)
```

`@=address,value` が書き込み、`@(address)` が読み取りです。

address には Cell の raw 16-bit pattern を使います。したがって負の Cell も `u16` と同じ bit pattern の address として扱われます。

```vtl
  @=-1,42
  A=@(-1)
```

この例の `-1` は storage address `65535` を指します。

### Value stack

ECVTL の procedure / primitive は共通の value stack を使います。

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

ECVTL の expression は、小さい parser と source role binding で構成されています。

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

現在の ECVTL では、binary operator ごとの precedence はありません。operator は同じ優先順位で**左結合**です。

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

という形です。`[` はこの途中状態を source から直接使うための surface です。

### `[=expr`: 結果を stack に残す

```vtl
  [=A+1
```

RHS を評価しますが、Write target へ渡しません。そのため結果は stack に残ります。

comma と組み合わせれば複数値を残せます。

```vtl
  [=10,20
```

### `target=[`: 既存 stack を target へ渡す

```vtl
  A=[
```

新しい RHS value を生成せず、現在の stack top を `A` の Write semantics へ渡します。

source-defined procedure に対しても同じです。

```vtl
  p=[
```

procedure が何値を消費するかは、その procedure の stack-effect contract に従います。

### RHS root の `[`

RHS の先頭に `[` を置くと、既存 stack top を最初の value として expression を続けられます。

```vtl
  A=[+2
```

既存 stack top が `10` なら、`A` は `12` になります。

この `[` は expression の任意位置で使える一般 value ではありません。RHS root の先頭に限定された structural surface です。

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

block は入れ子にでき、`[` は `]` で閉じます。`[` / `]` は空白または logical-line separator で独立した token にします。同じ行にも書けますが、複数行形式を推奨します。quoted string や comment 内の delimiter は認識されません。既存 stack surface の `[=expr`, `target=[`, RHS-root `[` は変わりません。body argument を要求していない位置に bare delimiter を置くことはできません。`|= ... =|` は anonymous block として使えません。

anonymous block は独立した procedure や runtime value ではありません。surrounding executable owner の code へ inline に構築され、numeric label namespace も surrounding owner と共有します。

statement target の `%=` は IF source command です。expression 中の `%` は従来どおり remainder operator であり、意味は変わりません。

### While `*=()`

`*=()` は直後に predicate と body の **2つの `[ ... ]` anonymous block** を取ります。どちらにも単独 statement の省略形はありません。

```vtl
  A=0
  *=()
  [
    [=A<10
  ]
  [
    A=A+1
  ]
```

第1 block の predicate は各反復の先頭で実行されます。その実行後、既存の `JumpIfZero` が現在の value stack top 1 Cell を condition として消費します。0なら終了し、0以外なら第2 block の body を実行して predicate に戻ります。

predicate の `( -- condition )` は programmer contract です。arity metadata や compile-time check はなく、condition が stack に無ければ通常の stack-underflow error になります。`*=()` 自体の `()` は通常 operand を生成しません。

While は stack を自動 cleanup しません。開始前からある値や predicate / body が余分に残した値は、共通の value stack 上にそのまま残ります。

statement の `*=()` は native While source command です。expression 内の `*` は従来どおり multiplication operator です。

## Source-defined 手続き

user-defined Write binding は、利用者向けには「手続き」と呼びます。

現在の surface は:

```vtl
  |=p
    ; procedure body
  p=|
```

current implementation では identity は 1 文字の lowercase ASCII letter です。

例:

```vtl
  ; ( address value -- )
  |=p
    V=[
    I=[
    @=I,V
  p=|

  p=10,42
```

procedure は caller と同じ value stack を使います。`p=10,42` は `10`, `42` を左から右へ stack に積んでから `p` を呼びます。

### Zero-operand invocation

operand を追加せず procedure を呼ぶには `()` を使います。

```vtl
  |=q
    ?="called"
  q=|

  q=()
```

`()` は UNIT value ではありません。「通常 operand を 0 個生成する」という source marker です。

callee が stack input を要求する場合、zero-operand call でも existing caller stack はそのまま見えます。空 stack で必要値が足りなければ通常の stack-underflow error になります。

### Definition order

source から procedure を公開するとき、call site は source-processing 時に binding を解決します。未定義 procedure の将来定義を runtime で再検索する仕組みはありません。

依存する procedure は、原則として callee を先に定義してください。

## Source role の用語

ECVTL の binding model は identity と source role の組です。

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

indentation は構文ではありませんが、user-facing ECVTL source では [#200](https://github.com/kkismd/vtlxx/issues/200) の規則を使います。

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
  |=q
    A=[
  ^=1
    ?=A
    A=A-1
    %=A [ #=1 ]
  q=|

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
  |=q
    ; consume the top value
    A=[
  q=|
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

## Example: Eight Queens

[`examples/eight_queens.vtl`](examples/eight_queens.vtl) は current ECVTL の user-facing example です。

次をまとめて使っています。

- A-Z register
- storage
- source-defined procedures for board writes, collision checks, output, and search
- shared value stack
- IF `%=` and While `*=()` with anonymous blocks
- decimal / newline output
- formatting / indentation rule

手続きは callee-first で定義し、stack effect と共有 register / storage の読み書きを source comment に記載しています。探索・衝突判定・解の表示では anonymous block と structured control を使い、numeric label / jump は使っていません。

実行:

```sh
cargo run -p vtlxx-poc-extended-classic-vtl --bin ecvtl -- \
  crates/poc/extended-classic-vtl/examples/eight_queens.vtl
```

92 個の解を列挙し、最後に解数 `92` を出力します。

[`examples/eight_queens_storage.vtl`](examples/eight_queens_storage.vtl) は、列ごとの候補と解数を storage に置き、現在の候補だけを短時間 register に読み出す別例です。同じ92解を逐一表示します。

## Example: 固定迷路の DFS

[`examples/maze_dfs.vtl`](examples/maze_dfs.vtl) は、固定 4×4 迷路を再帰を使わずに探索します。迷路、訪問済み状態、現在の経路、各深さで次に試す方向を別々の indexed storage 領域に置きます。右から進んだ cell 2 の行き止まりで一度戻り、goal への経路の cell 数 `7` を表示します。

```sh
cargo run -p vtlxx-poc-extended-classic-vtl --bin ecvtl -- \
  crates/poc/extended-classic-vtl/examples/maze_dfs.vtl
```

## Example: 固定 token 列の RPN evaluator

[`examples/rpn_evaluator.vtl`](examples/rpn_evaluator.vtl) は source 自身が indexed storage に `3 4 ADD 2 MUL 7 SUB` を初期化し、共有 value stack を operand stack として評価して `7` を表示します。operator 手続きはそれぞれ `( lhs rhs -- result )` で、token は program counter で順に読みます。

```sh
cargo run -p vtlxx-poc-extended-classic-vtl --bin ecvtl -- \
  crates/poc/extended-classic-vtl/examples/rpn_evaluator.vtl
```

## ECVTL source を書くときの確認事項

新しい ECVTL source を追加・変更するときは、少なくとも次を確認してください。

1. current Tracker で、その surface が `main` に実装済みか確認する。
2. statement 内へ whitespace を入れない。
3. binary operator の通常の precedence を仮定せず、必要なら `(...)` を使う。
4. procedure の stack effect を comment で明示する。
5. procedure 呼び出しは callee-first の定義順を守る。
6. numeric label は executable owner ごとの local namespace として扱う。
7. indentation は 2 ASCII spaces 単位で #200 の owner rule に従う。
8. 未実装の関数・拡張プロパティ・拡張演算子を推測で書かない。
9. current code / tests / accepted ADR と README が食い違う場合は、README を正本として扱わず差分を確認する。
