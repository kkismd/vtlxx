# AGENTS.md

この文書は、VTLXX repository で設計・実装・テスト・レビューを行うエージェント向けの共通ガードレールを定める。

## プロジェクトの位置付け

VTLXX は、極小の実行系から上位の言語層を bootstrap する構造を検証する実験的な言語処理系である。

Rust 実装は最終ターゲットそのものではなく、意味論と bootstrap 構造を検証するための reference implementation / executable specification として扱う。

将来の低水準実装へ移植可能な程度に単純な構造を優先し、Rust 固有の高度な抽象化を必要性が明確になる前に導入しない。

## 正本と確認順序

実装または設計変更に着手する前に、次を確認する。

1. 対応する issue
2. 親 Tracker / マイルストーン issue
3. 根拠となる採用済み ADR・仕様
4. 前提 issue・PR
5. 現在のコード・テスト

設計判断は採用済み ADR を正本とする。現在の実装事実はコードとテストを正本とする。

現在の開発段階、対象 stage、issue 系列、対象範囲、非目標その他の変化しやすい情報は、対応する Tracker / マイルストーン issue を正本とする。

ChatGPT や他のエージェントの過去の発言だけを、確定仕様または現在状態の根拠として扱わない。

文書、ADR、コード、テストの間に矛盾がある場合は、暗黙にどれかを正しいものとして扱わず、矛盾を明示して解消する。

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

## テスト

新規または変更された契約は、具体的なテストまたは確認方法へ対応付ける。

「既存テストが通る」「CI が通る」だけを、新しい契約の確認方法として扱わない。

既存テストを変更・置換・削除する場合は、そのテストが固定していた意味論や回帰点を失っていないことを確認する。

## 現在の仕様を確認するとき

この文書には、開発の進行で変化する stage 固有の仕様や現在の issue 系列を固定しない。

作業対象の現在状態は、GitHub 上の対応する Tracker / マイルストーン issue、関連 issue、採用済み ADR、コード、テストを確認すること。
