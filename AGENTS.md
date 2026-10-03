# AGENTS.md

この文書は、VTLXX repository で設計・実装・テスト・レビューを行うエージェント向けの共通ガードレールを定める。

## プロジェクトの位置付け

VTLXX は、極小の実行系から上位の言語層を bootstrap する構造を検証する実験的な言語処理系である。

Rust 実装は最終ターゲットそのものではなく、意味論と bootstrap 構造を検証するための reference implementation / executable specification として扱う。

将来の低水準実装へ移植可能な程度に単純な構造を優先し、Rust 固有の高度な抽象化を必要性が明確になる前に導入しない。

VTLXX 固有の bootstrap 設計原則は、一般的な言語処理系、Rust、ソフトウェア設計の慣習より優先する。一般的には妥当な方式でも、bootstrap kernel の最小性、stage 間責務、移植可能性その他の本書の原則に反する場合は採用しない。

## 正本と確認順序

実装または設計変更に着手する前に、次を確認する。

1. 対応する issue
2. 親 Tracker / マイルストーン issue
3. 根拠となる採用済み ADR・仕様
4. 前提 issue・PR
5. 現在のコード・テスト

repository root の `AGENTS.md` に加え、変更対象パスにより近い `AGENTS.md` が存在する場合は、それも適用する。関連文書は変更対象と issue が要求するものを確認し、過去 stage や別対象の資料を現在仕様として扱わない。

VTLXX repository 全体の概要と主要文書への入口は root `README.md` を参照する。ECVTL の source surface、実行方法、記述例、formatting rule を扱う場合は、`crates/poc/extended-classic-vtl/README.md` を実用入口として確認する。これらの README は探索と実用上の案内であり、設計判断・現在の開発段階・現在の実装事実の正本を置き換えない。

設計判断は採用済み ADR を正本とする。現在の実装事実はコードとテストを正本とする。

現在の開発段階、対象 stage、issue 系列、対象範囲、非目標その他の変化しやすい情報は、対応する Tracker / マイルストーン issue を正本とする。

ChatGPT や他のエージェントの過去の発言だけを、確定仕様または現在状態の根拠として扱わない。

文書、ADR、コード、テストの間に矛盾がある場合は、暗黙にどれかを正しいものとして扱わず、矛盾を明示して解消する。

情報が矛盾する場合は、原則として次の順で現在状態と適用範囲を確認する。

1. GitHub 上の現在状態
2. 対象に適用される `AGENTS.md`
3. 現在の Tracker / マイルストーン issue
4. より新しい issue・PR
5. 採用済み ADR
6. repository 内の仕様文書
7. 実装コード・テスト
8. 過去資料・過去の会話やエージェントの発言

この順序は機械的に後順位の情報を無視するためではなく、現在状態と適用範囲を確認するための基準とする。意味論上の設計判断は採用済み ADR、現在の実装事実はコードとテストを正本とする原則を維持し、矛盾が解消しない場合は差分と採用根拠を明示する。

## 用語・命名

* issue、ADR、仕様その他の文書で新しい概念を命名するときは、英単語を組み合わせた造語を使用せず、日本語で表現する。
* 「境界」のような抽象語は、意味を一語にまとめる必要がある場合に限って用いる。責務分担、有効範囲、所有主体、完成条件、公開時点など、意図をより具体的な日本語で表現できる場合は、そちらを優先する。
* IT・計算機科学分野で日本語として定着しているカタカナ語がある場合は、それを優先する。それ以外は一般的な日本語表現を使用する。
* 既存のコードで既に定義されている型名、モジュール名、関数名、crate 名その他のコード上の識別子は、この方針の対象外とし、既存の英名を日本語へ翻訳しない。
* 新しいコード上の識別子そのものの命名については、既存コードおよび言語ごとの命名規則に従う。

## bootstrap 設計の原則

* 下位 stage は、完成された便利な言語ではなく、次の言語層を構築するための最小機構として評価する。
* 高水準の便利さだけを理由に、下位 stage や bootstrap kernel を複雑化しない。
* 複数 stage で primitive や意味論を不必要に二重実装しない。
* parser と execution semantics を分離する。
* primitive は source syntax を直接解釈しない。
* stack effect で説明できるものは、可能な限り stack effect として定義する。
* 下位機構へ上位構文固有の都合を安易に持ち込まない。
* machine-level value は、特別扱いが不要なら通常の Cell として扱う方向を優先する。
* 上位構文は、可能な限り下位 machine の既存 primitive / semantics へ変換できる形を優先する。
* 未決定事項を実装上の都合だけで確定仕様にしない。
* bootstrap 可能性、意味論の単純さ、移植可能性を、実装上の技巧より優先する。

これらは長期的な設計原則であり、具体的な stage 構成、primitive 集合、memory model、dictionary、control flow 等の現在の仕様をこの文書だけから推測してはならない。

## 下位 stage の設計・レビュー時の判断基準

下位 stage へ新しい機構を追加・維持する必要性を検討するときは、少なくとも次を確認する。

* 次の言語層を構築するために本当に必要か。
* より小さい機構で代替できないか。
* 既存 primitive や user-defined word 等で代替できないか。
* parser 側または上位 stage へ置けないか。
* C / assembly でも自然に表現できるか。
* stack effect と machine state の変化で意味を説明できるか。

syntax-specific な専用機構を追加する前に、既存の小さく直交した機構の合成で表現できないか確認する。

特定用途向け primitive や下位 stage の専用機構を追加する場合は、既存 primitive、user-defined word、parser 側の処理、上位 stage の機構等の組み合わせでは十分に表現できない理由を、issue / ADR / PR のいずれかに明記する。

「便利である」「Rust では実装しやすい」「一般的な言語処理系ではよく使われる」ことだけを、下位 stage へ機構を置く根拠にしない。

## 実装上のガードレール

issue または採用済み ADR に根拠がない限り、次のような変更を先回りして導入しない。

* 高度な trait hierarchy や generic abstraction
* AST 中心の下位 stage 設計
* GC、arena その他の複雑な memory management
* 長い識別子や高水準構文のための下位機構
* parser と primitive の直接結合
* stage ごとに別実装された同一 semantics

具体的な型構成、関数分割、内部 API のうち、意味論・責務分担・公開境界・後続実装へ影響しない局所的な物理詳細は実装者に委ねる。

一方、実装者が責務分割そのものを新たに設計しなければ着手できない状態を「実装詳細」として残さない。新しい crate、subsystem、PoC 等をコードベースがない状態から開始する場合は、必要に応じて実装先、基本 module 構成、主要型、概念 API、visibility、状態所有、error 分類、前後 issue との責務分担まで実装 issue で具体化する。

たとえば次のような事項は、通常は実装者へ委ねてよい。

* ID の具体的な整数幅
* private struct の field 配置
* private helper 関数の分割
* patch table や linear table の内部表現
* allocation capacity
* test fixture helper の形
* 意味論へ露出しない内部識別子の生成方式

ただし、次に影響する変更は実装都合だけで決めない。

* primitive の stack effect
* parser と execution の責務分担
* Cell の意味
* address / memory model
* dictionary lookup / dispatch
* call / return
* data stack / return stack の役割
* 副作用順序
* error 時の machine state
* stage 間で共有する semantics

既存 ADR または仕様から一意に導けない重要な設計判断が必要になった場合は、その場で確定せず、調査・検討 issue または ADR issue へ分離する。

## issue と PR

原則として 1 issue - 1 PR とする。

issue は、次の種別を明記する。

* **Tracker / マイルストーン**: 開発段階を管理し、目的、対象範囲、子 issue、依存関係、完了条件を整理する。
* **調査・検討**: 現状確認、選択肢比較、必要条件整理、検証、ADR の判断材料作成を行う。
* **ADR**: 言語仕様、bootstrap 構造、VM semantics その他の重要な設計判断を記録する。
* **実装**: 採用済み仕様または ADR に基づきコード、テスト、文書等を変更する。

原則として次の順で進める。

```text
Tracker / マイルストーン
    ↓
調査・検討
    ↓
ADR
    ↓
実装
```

一つの issue に調査、重要な設計判断、大規模な実装を混在させない。調査・検討 issue では原則としてコード変更を行わない。重要な設計判断は ADR で確定してから実装へ進む。

### Tracker / マイルストーン issue

原則として次を含める。

1. 背景
2. 目的
3. このマイルストーンで決定・検証すること
4. 決めないこと
5. 前提となる ADR・設計原則
6. 子 issue と依存関係
7. 完了条件
8. 現在の進捗
9. 次のマイルストーンへ送る事項

対象 stage、現在の候補仕様、issue 系列、非目標等の変化しやすい情報は Tracker / マイルストーン issue に置く。詳細な設計判断は必要に応じて調査・検討 issue や ADR issue へ分離する。

### 調査・検討 issue

原則として次を含める。

1. 背景
2. 目的
3. 確認対象
4. 対象外
5. 前提条件
6. 確認方法・判断基準
7. 比較する選択肢
8. 調査結果
9. 成果物・完了条件
10. 関連 Tracker・issue・ADR
11. ADR で決定すべき事項
12. 後続 issue へ分割すべき事項

調査結果をそのまま確定仕様とせず、重要な設計判断は原則として ADR へ分離する。

調査・検討 issue には `Research` を付け、結果に応じて次の分類ラベルを1つ付ける。

* ADR で決定すべき事項を含む → `Research_needs_ADR`
* ADR を必要とせず完結する → `Research_no_ADR`
* より新しい調査に置き換えられた → `Research_superseded`

`Research_needs_ADR` は ADR が未作成であることではなく、その調査結果が ADR の判断材料となることを示す。後続 ADR 作成後も残してよい。

### ADR issue

原則として次を含める。

1. 背景・問題
2. 決定状態
3. 決定内容
4. 採用理由
5. 代替案
6. stage 間構造と bootstrap への影響
7. 意味論上の不変条件
8. 実装・移行方針
9. 関連 Tracker・issue・ADR
10. 未決定事項

決定状態は `提案中`、`採用`、`置換済み`、`撤回`、`却下` のいずれかとし、本文を正本とする。ADR issue には決定状態に対応するラベルを1つだけ付ける。

* `提案中` → `ADR_proposed`
* `採用` → `ADR_accepted`
* `置換済み` → `ADR_superseded`
* `撤回` → `ADR_withdrawn`
* `却下` → `ADR_rejected`

ラベルは検索・一覧化用の補助情報であり、本文の決定状態を置き換えない。

既存 ADR を変更するときは暗黙に書き換えず、新しい ADR から置換対象と理由を明記する。

### 実装 issue

実装着手前に、対象パスに適用される `AGENTS.md`、issue 本文、親 Tracker / マイルストーン、根拠となる ADR・仕様、前提 issue・PR、既存実装、関連テストを確認する。

実装 issue は、対応する 1 PR を実装者が重要な設計判断なしに着手でき、reviewer が issue 本文を基準として実装構造まで確認できる粒度まで具体化する。

原則として、一つの実装 issue は独立して完成・検証可能な主要責務を一つ扱う。実行基盤、成果物構築、公開、parser、source processing、frontend、end-to-end proof 等がそれぞれ独立した不変条件や責務を持つ場合は、一つの大きな実装 issue にまとめず、前後関係を明示した複数の実装 issue へ分割する。

原則として次を含める。

0. 実装着手時の確認事項
1. 背景・目的
2. 対象範囲
3. 対象外
4. 仕様・設計上の根拠
5. 実装内容と制約
6. 受け入れ条件
7. テスト方針
8. 関連 Tracker・issue・ADR
9. 意味論・互換性・回帰点
10. 未決定事項

新しい crate、subsystem、PoC 等をコードベースが存在しない状態から開始する場合は、必要に応じて次も具体化する。

* 実装先の repository path
* crate / package 等の成果物単位
* 基本的な file / module 構成
* 各 module の責務
* 主要な型とその役割
* module 間を接続する概念 API
* public / private / crate-private の方針
* 永続状態と一時状態の所有主体
* error の分類と失敗時の状態契約
* 前提 issue と後続 issue との責務分担
* 推奨する実装順序
* テスト配置
* 必須検証コマンド

型名、関数名、API 名の完全な signature まで固定する必要はないが、実装者が責務分割そのものを新たに設計しなければ着手できない状態を残さない。

一方、ID の具体幅、private field 配置、private helper 分割、patch table の物理表現など、意味論・責務分担・公開境界・後続設計へ影響しない局所的な物理詳細は実装者に委ねてよい。

受け入れ条件は具体的に検証可能な形で記述し、テスト方針では各条件と確認方法を対応付ける。正常系だけでなく、error path、失敗時原子性、公開前後、ownership、binding 等の重要な negative case も必要に応じて受け入れ条件へ含める。

issue にない重要な設計判断が必要になった場合は独自に確定せず、既存 ADR や仕様から一意に導けないものは調査・検討 issue または ADR issue へ分離する。

#### 実装 issue の着手前レビュー

着手前レビューでは少なくとも次を確認する。

* 1 issue - 1 PR として scope が十分に小さいか。
* 実装先と主な変更対象が明確か。
* 各 module / component の責務が明確か。
* 主要型・概念 API・visibility の方針が、実装開始に十分な程度まで具体化されているか。
* 前提実装と後続 issue の責務を先取りしていないか。
* 実装者が未決定の重要設計判断を行う必要が残っていないか。
* 各受け入れ条件が具体的な test / inspection 方法へ対応しているか。
* error path、失敗時原子性、公開前後、ownership、binding 等の重要な negative case が受け入れ条件に含まれているか。
* 必須検証コマンドが明示されているか。

重要な設計判断が残っている場合は着手可能とせず、必要な調査・ADRへ戻す。単に private な物理表現だけが未指定である場合は、それを理由に実装を止めない。

### PR 作成

PR 本文には原則として次を含める。

1. 対応 issue と親 Tracker
2. 変更目的・内容
3. 対象外
4. 仕様・ADR との対応
5. 受け入れ条件との対応
6. テスト結果
7. 意味論上の注意点
8. 未解決事項

PR では issue にない重要な設計判断を暗黙に追加しない。scope 外の変更を便宜的に含めない。

### PR レビュー

レビュー前に、最新 head SHA、対応 issue、親 Tracker / マイルストーン、根拠となる ADR・仕様、適用される `AGENTS.md`、diff、関連テスト・CI を確認する。

少なくとも次を確認する。

* issue と Tracker の目的・受け入れ条件を満たすか。
* 対象外の変更がないか。
* ADR、仕様、本書、既存実装と矛盾しないか。
* 必要なテストがあるか。
* issue にない重要な設計判断を追加していないか。
* 未決定事項を実装都合で確定していないか。
* scope を超えた変更や不要な先回り実装がないか。
* issue で定めた crate / file / module 責務から不必要に逸脱していないか。
* public API が後続実装の便宜だけを理由に拡大していないか。
* source-level identity や parser artifact が runtime へ漏れていないか。
* completed / published な成果物と未完成成果物が区別されているか。
* owner-local な値や handle が所有者を越えて利用できないか。
* early binding 等の解決済み情報が runtime で再解決されていないか。
* 失敗する単一操作が契約外の partial mutation を残さないか。
* error 後に保持すべき状態と破棄すべき状態が仕様どおりか。
* test-only bypass が production 経路の不変条件を破っていないか。
* 既存テストを変更した場合、そのテストが固定していた意味論上の回帰点を維持しているか。

意味論や重要な契約に影響する問題を指摘するときは、問題だけでなく、守るべき不変条件、責務分担、避けるべき方式、影響する ADR・仕様、確認すべきテストを示す。

内部実装詳細については、それが issue / ADR の不変条件を守り、外部契約へ露出せず、後続設計を不必要に固定しない限り、reviewer の好みだけで別方式を要求しない。

レビュー結果は PR 上に comment として記録する。author と reviewer が同一アカウントの場合は approve / request changes を使わない。

レビューコメントは原則として次の形式とする。

```text
レビュー対象: <head SHA>

ブロッカー: あり / なし

## 仕様・ADRとの整合性
...

## 修正が必要な点
...

## 参考コメント
...

## テスト・検証
...

## scope
...

## 結論
merge可 / 修正後に再レビュー
```

head SHA が変化した場合、以前のレビュー結果を新しい head へ自動的に引き継がない。再レビューでは前回の blocking 指摘の解消を明示的に確認する。

## Git workflow

* `main` へ直接 commit せず、branch と PR を経由する。
* エージェントは自分で PR を merge しない。
* issue の close は PR merge またはユーザー判断に委ねる。
* branch 削除、force push、reset、rebase、履歴改変では、親子関係、tree 差分、merge 構造、他 branch からの参照、未マージ差分を確認する。
* default branch の履歴変更前には復元可能な backup ref を作成する。
* GitHub の merged / unmerged 表示や commit message だけで安全性を判断しない。
* 不可逆性の高い操作では、対象と影響範囲を明示する。

## テスト

新規または変更された契約は、具体的なテストまたは確認方法へ対応付ける。

「既存テストが通る」「CI が通る」だけを、新しい契約の確認方法として扱わない。

既存テストを変更・置換・削除する場合は、そのテストが固定していた意味論や回帰点を失っていないことを確認する。

## 現在の仕様を確認するとき

この文書には、開発の進行で変化する stage 固有の仕様や現在の issue 系列を固定しない。

作業対象の現在状態は、GitHub 上の対応する Tracker / マイルストーン issue、関連 issue、採用済み ADR、コード、テストを確認すること。
