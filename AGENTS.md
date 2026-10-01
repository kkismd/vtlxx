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

設計判断は採用済み ADR を正本とする。現在の実装事実はコードとテストを正本とする。

現在の開発段階、対象 stage、issue 系列、対象範囲、非目標その他の変化しやすい情報は、対応する Tracker / マイルストーン issue を正本とする。

ChatGPT や他のエージェントの過去の発言だけを、確定仕様または現在状態の根拠として扱わない。

文書、ADR、コード、テストの間に矛盾がある場合は、暗黙にどれかを正しいものとして扱わず、矛盾を明示して解消する。

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

具体的な型構成、関数分割、内部 API は、意味論上の契約に影響しない限り実装者に委ねる。

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

調査・検討 issue では原則としてコード変更を行わない。重要な設計判断は ADR で確定してから実装へ進む。

実装時は issue の対象範囲、対象外、受け入れ条件、テスト方針を守る。scope 外の変更を便宜的に含めない。

PR では、issue にない重要な設計判断を暗黙に追加しない。

## Git workflow

* `main` へ直接 commit せず、branch と PR を経由する。
* エージェントは自分で PR を merge しない。
* issue の close は PR merge またはユーザー判断に委ねる。

## テスト

新規または変更された契約は、具体的なテストまたは確認方法へ対応付ける。

「既存テストが通る」「CI が通る」だけを、新しい契約の確認方法として扱わない。

既存テストを変更・置換・削除する場合は、そのテストが固定していた意味論や回帰点を失っていないことを確認する。

## 現在の仕様を確認するとき

この文書には、開発の進行で変化する stage 固有の仕様や現在の issue 系列を固定しない。

作業対象の現在状態は、GitHub 上の対応する Tracker / マイルストーン issue、関連 issue、採用済み ADR、コード、テストを確認すること。
