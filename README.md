# VTLXX

VTLXX は、Classic VTL の簡潔なプログラミングモデルを発展させた、小型コンピュータ向けの実験的なプログラミング言語およびコンパイラです。従来 **Extended Classic VTL (ECVTL)** と呼んでいた処理系を正式な VTLXX として継承しています（[採用 ADR #540](https://github.com/kkismd/vtlxx/issues/540)）。

Rust の参照実装と actual NMOS 6502 向けネイティブコンパイラがあり、共通の言語意味論、単純な構造、限られた資源での実行可能性を重視します。M01 の独立したブートストラップ計画は廃番とし、自己ホストを現行の必須目標にはしていません。歴史的な M01 の issue・ADR・実装・PR は保存します。

## 読み始める場所

- repository で設計・実装・テスト・レビューを行うときの共通ルールは [`AGENTS.md`](AGENTS.md) を確認してください。
- VTLXX の現行 source surface、実行方法、記述例、formatting rule は [実用 README](crates/vtlxx/README.md) を参照してください。
- **現在の実装名**: Rust package は `vtlxx`、path は `crates/vtlxx/`、Rust CLI は `vtlxx`、6502 CLI は `vtlxx6502` です。実行コマンドは実用 README に従ってください。
- 現在の開発段階、対象範囲、進捗は対応する GitHub Tracker / マイルストーン issue を確認してください。名称・開発方針の変更は [ADR #540](https://github.com/kkismd/vtlxx/issues/540) を正本とします。
- 重要な個別言語設計は従来ECVTLの現在有効な採用済みADR、現在の実装事実はコードとテストを正本とします。

README は入口と実用上の案内です。Tracker、ADR、コード、テストと食い違う場合は README を確定仕様として扱わず、対象に適用される `AGENTS.md` の確認順序に従って現在状態を確認してください。
