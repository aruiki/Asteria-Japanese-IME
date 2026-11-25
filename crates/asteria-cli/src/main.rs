//! Asteria IME 統合ツール (`asteria-tools`)。

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Result;
use clap::{Parser, Subcommand};

use asteria_assets::{capture, figures, icons, installer};
use asteria_common::paths::find_repo_root;
use asteria_corpus::{collocations, eval_sets, frequency, initial_ranker, lexicon, training_prep};
use asteria_eval::{acceptance, baseline, bench, large_input, tools};
use asteria_release::{
    check_release::{run_check_release, CheckReleaseOpts},
    installer_smoke::{run_installer_smoke, SmokeOpts},
    local_install_check::{run_local_install_check, LocalInstallOpts},
    probe::{run_installed_probe, run_live_conversion_check},
    release_name::{release_name, SimpleDate},
    release_tag,
};

#[derive(Parser)]
#[command(
    name = "asteria-tools",
    about = "Asteria IME 統合開発・配布・評価・資産ツール"
)]
struct Cli {
    /// リポジトリルート（省略時はカレントディレクトリまたは実行ファイルから探索）
    #[arg(long, global = true)]
    repo_root: Option<PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 次の公開・候補タグを採番する (`release_tag.py`)
    ReleaseTag {
        #[arg(long, default_value = "stable")]
        channel: String,
        #[arg(
            long = "repo",
            alias = "repo-url",
            default_value = "https://github.com/aruiki/Asteria-Japanese-IME.git"
        )]
        repo_url: String,
        #[arg(long)]
        notes: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// リリース表示名を生成する (`release_name.py`)
    ReleaseName {
        #[arg(long)]
        tag: String,
        #[arg(long)]
        date: Option<String>,
    },
    /// 公開・ローカルビルド MSI の同梱物と版を検証する (`check_release.py`)
    CheckRelease {
        #[arg(long, default_value = "latest")]
        tag: String,
        #[arg(long, default_value = "tmp-release-check")]
        work: PathBuf,
        #[arg(long)]
        local: Option<PathBuf>,
        #[arg(long)]
        previous_local: Option<PathBuf>,
        #[arg(long)]
        require_generator: bool,
        #[arg(long)]
        require_collocations: bool,
        #[arg(long)]
        require_short_lexicon: bool,
    },
    /// 使い捨て Windows ランナー向けインストーラスモーク試験 (`installer_smoke.py`)
    InstallerSmoke {
        #[arg(long)]
        msi: PathBuf,
        #[arg(long, default_value = "installer-smoke")]
        out: PathBuf,
        #[arg(long)]
        acceptance: bool,
        #[arg(long)]
        duplicate_stress: bool,
        #[arg(long)]
        browser_duplicate_stress: bool,
        #[arg(long)]
        reinstall: bool,
    },
    /// ローカル PC の段階的候補 MSI 更新・復帰検査 (`local_install_check.py`)
    LocalInstallCheck {
        #[arg(long)]
        stage: String,
        #[arg(long, default_value = "target/local-install-check")]
        out: PathBuf,
        #[arg(long)]
        candidate: Option<PathBuf>,
        #[arg(long)]
        rollback: Option<PathBuf>,
        #[arg(long)]
        expected_dir: Option<PathBuf>,
        #[arg(long)]
        restore_profile: bool,
        #[arg(long)]
        skip_click: bool,
        #[arg(long)]
        python: Option<String>,
        #[arg(long)]
        ui: bool,
    },
    /// 導入済み Asteria 実機プローブ (`installed_probe.py`)
    InstalledProbe {
        #[arg(long, default_value = "target/installed-probe")]
        out: PathBuf,
        #[arg(long, default_value_t = 1200)]
        wait_ms: u64,
        #[arg(long)]
        live: bool,
        keys: Vec<String>,
    },
    /// ライブ変換有効時の実機プローブ (`live_conversion_check.py`)
    LiveConversionCheck {
        #[arg(long, default_value = "target/live-conversion-check")]
        out: PathBuf,
        #[arg(long, default_value_t = 1500)]
        wait_ms: u64,
    },

    /// 連語パッケージの検証とステージング (`training/collocations/stage.py`)
    CollocationsStage {
        #[arg(long)]
        baseline: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        cache: Option<PathBuf>,
        #[arg(long)]
        ajimee: Option<PathBuf>,
        #[arg(long)]
        usage_dir: Option<PathBuf>,
        #[arg(long)]
        cforms: Option<PathBuf>,
    },
    /// Wikipedia コーパスから fast_collocations を構築 (`build_fast_collocations.py`)
    BuildFastCollocations {
        #[arg(long)]
        corpus: PathBuf,
        #[arg(long)]
        corpus_manifest: PathBuf,
        #[arg(long)]
        baseline: PathBuf,
        #[arg(long)]
        exclude: Vec<PathBuf>,
        #[arg(long)]
        out: PathBuf,
    },
    /// 用例辞書から連語を構築 (`build_usage_collocations.py`)
    BuildUsageCollocations {
        #[arg(long)]
        baseline: PathBuf,
        #[arg(long)]
        usage: PathBuf,
        #[arg(long)]
        cforms: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// 短文語彙・頻度集計用ソースを取得 (`training/lexicon/fetch_sources.py`)
    LexiconFetchSources {
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        cache: Option<PathBuf>,
        #[arg(long)]
        config: PathBuf,
    },
    /// 事前パッケージ済み短文語彙をステージング (`training/lexicon/stage_package.py`)
    LexiconStagePackage {
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        source: Option<PathBuf>,
        #[arg(long)]
        manifest: Option<PathBuf>,
    },
    /// Sudachi トークナイザ資材をステージング (`training/lexicon/stage_tokenizer.py`)
    LexiconStageTokenizer {
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        cache: Option<PathBuf>,
    },
    /// Sudachi + Mozc 辞書から short_lexicon を構築 (`training/lexicon/build_lexicon.py`)
    LexiconBuild {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        mozc: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        config: PathBuf,
    },
    /// コーパス頻度収集 (`training/lexicon/collect_frequency.py`)
    LexiconCollectFrequency {
        #[arg(long)]
        source: PathBuf,
        #[arg(long)]
        runtime_dir: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        exclude: Vec<PathBuf>,
        #[arg(long, default_value_t = 4)]
        shards: usize,
        #[arg(long, default_value_t = 0)]
        limit_bytes: u64,
    },
    /// 頻度ベース short_lexicon 構築 (`training/lexicon/build_frequency_lexicon.py`)
    LexiconBuildFrequency {
        #[arg(long)]
        candidate_dir: PathBuf,
        #[arg(long)]
        frequency_dir: PathBuf,
        #[arg(long)]
        mozc: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// 頻度ベース連語構築 (`training/lexicon/build_frequency_collocations.py`)
    CollocationsBuildFrequency {
        #[arg(long)]
        frequency_dir: PathBuf,
        #[arg(long)]
        baseline: PathBuf,
        #[arg(long)]
        runtime_path: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// 診断語彙セット構築 (`training/lexicon/build_diagnostic.py`)
    LexiconBuildDiagnostic {
        #[arg(long)]
        candidate_dir: PathBuf,
        #[arg(long)]
        frequency_dir: PathBuf,
        #[arg(long)]
        mozc: PathBuf,
        #[arg(long)]
        exclude: Vec<PathBuf>,
        #[arg(long)]
        out: PathBuf,
    },
    /// 対比学習用生成モデルの取得 (`training/contrastive/fetch_generator.py`)
    FetchGenerator {
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        source: Option<PathBuf>,
        #[arg(long)]
        manifest: Option<PathBuf>,
    },
    /// 生成モデル群バンドルのステージング (`training/contrastive/stage_generator.py`)
    StageGenerator {
        #[arg(long)]
        generator: PathBuf,
        #[arg(long)]
        base_dir: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// 観測・汎用コーパスから対比学習ペアを作成 (`training/contrastive/native_pairs.py`)
    NativePairs {
        #[arg(long)]
        observed: PathBuf,
        #[arg(long)]
        general: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },

    /// 評価セット生成 (`build_eval_sets.py`)
    BuildEvalSets {
        #[arg(long)]
        sources: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        dict: PathBuf,
        #[arg(long, default_value_t = 500)]
        judge_per_domain: usize,
        #[arg(long, default_value_t = 300)]
        dev_per_domain: usize,
    },
    /// 文脈選択・長文セット生成 (`build_context_choice.py`)
    BuildContextChoice {
        #[arg(long)]
        sources: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        dict: PathBuf,
        #[arg(long, default_value_t = 150)]
        judge_pairs: usize,
        #[arg(long, default_value_t = 150)]
        dev_pairs: usize,
        #[arg(long, default_value_t = 60)]
        judge_passages: usize,
        #[arg(long, default_value_t = 60)]
        dev_passages: usize,
        #[arg(long, default_value_t = 4)]
        passage_len: usize,
    },
    /// 前文依存最小対生成 (`build_context_pairs.py`)
    BuildContextPairs {
        #[arg(long)]
        sources: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        dict: PathBuf,
        #[arg(long, default_value_t = 150)]
        judge_pairs: usize,
        #[arg(long, default_value_t = 100)]
        dev_pairs: usize,
    },
    /// かな表記比率集計 (`build_kana_preference.py`)
    BuildKanaPreference {
        #[arg(long)]
        sources: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        dict: PathBuf,
        #[arg(long, default_value_t = 20)]
        min_count: usize,
    },
    /// Wikipedia 固有名詞辞書生成 (`build_name_lexicon.py`)
    BuildNameLexicon {
        #[arg(long)]
        articles: PathBuf,
        #[arg(long)]
        exclude: Vec<PathBuf>,
        #[arg(long)]
        mozc_dict: PathBuf,
        #[arg(long)]
        short_lexicon: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        dict: PathBuf,
        #[arg(long, default_value_t = 0)]
        cost_offset: i64,
    },
    /// Sonnet 文脈組セット整形 (`import_sonnet_choice.py`)
    ImportSonnetChoice {
        #[arg(long)]
        src: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        dict: PathBuf,
    },
    /// Wikipedia 新規記事取得 (`fetch_wikipedia_new_articles.py`)
    FetchWikipediaNewArticles {
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value = "2024-03")]
        start: String,
        #[arg(long, default_value = "2026-10")]
        end: String,
        #[arg(long, default_value_t = 120)]
        per_month: usize,
    },
    /// 日常表記 C++ `.inc` テーブル生成 (`gen_notation_table.py`)
    GenNotationTable {
        #[arg(long)]
        tsv: PathBuf,
        #[arg(long)]
        meta: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// 文脈既定手がかり `.inc` テーブル生成 (`gen_context_defaults.py`)
    GenContextDefaults {
        #[arg(long)]
        json: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// ATOK 調整用 3 セット書き出し (`make_atok_sets.py`)
    MakeAtokSets {
        #[arg(long)]
        out: PathBuf,
    },

    /// 初期順位: コーパス分割・3-gram モデル学習 (`initial_ranker/prepare.py`)
    RankerPrepare {
        #[arg(long)]
        corpus: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        exclude: Vec<PathBuf>,
        #[arg(long)]
        source_commit: String,
        #[arg(long, default_value_t = 2000)]
        train_limit: usize,
    },
    /// 初期順位: ラティス観測収集 (`initial_ranker/collect.py`)
    RankerCollect {
        #[arg(long)]
        exe: PathBuf,
        #[arg(long)]
        data: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        source_commit: String,
    },
    /// 初期順位: 重み・余裕幅の校正 (`initial_ranker/calibrate.py`)
    RankerCalibrate {
        #[arg(long)]
        observations: PathBuf,
        #[arg(long)]
        model: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// 初期順位: 独立データへの適用評価 (`initial_ranker/evaluate.py`)
    RankerEvaluate {
        #[arg(long)]
        observations: PathBuf,
        #[arg(long)]
        model: PathBuf,
        #[arg(long)]
        weight: i64,
        #[arg(long)]
        margin: i64,
        #[arg(long)]
        out: PathBuf,
    },

    /// オフラインベースライン変換評価 (`mozc/eval_baseline.py`)
    EvalBaseline {
        #[arg(long)]
        converter_main: PathBuf,
        #[arg(long)]
        data: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        profile: Option<PathBuf>,
        #[arg(long)]
        context: bool,
        #[arg(long, default_value_t = 30.0)]
        timeout: f64,
        #[arg(long)]
        stderr_out: Option<PathBuf>,
        #[arg(long)]
        min_acc: Option<f64>,
        #[arg(long, default_value = "")]
        converter_source_commit: String,
        #[arg(long)]
        case_traces: Option<PathBuf>,
        #[arg(long)]
        request_traces: Option<PathBuf>,
    },
    /// 予測サジェスト評価 (`mozc/eval_predict.py`)
    EvalPredict {
        #[arg(long)]
        converter_main: PathBuf,
        #[arg(long)]
        data: PathBuf,
        #[arg(long, default_value_t = 2)]
        typed_len: usize,
        #[arg(long, default_value_t = 5)]
        top_k: usize,
        #[arg(long, default_value_t = 4)]
        min_len: usize,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// セッション経路評価 (`mozc/tools/session_eval.py`)
    SessionEval {
        exe: PathBuf,
        #[arg(long)]
        data: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        context: bool,
        #[arg(long)]
        models: Option<PathBuf>,
        #[arg(long, default_value = "llm")]
        ai_state: String,
        #[arg(long, default_value_t = 15.0)]
        warm: f64,
        #[arg(long, default_value_t = 0)]
        limit: usize,
        #[arg(long)]
        immediate_space: bool,
        #[arg(long)]
        space_gap: Option<f64>,
        #[arg(long)]
        live: bool,
        #[arg(long)]
        timeline: bool,
        #[arg(long = "config")]
        configs: Vec<String>,
        #[arg(long = "env")]
        envs: Vec<String>,
    },
    /// 表記ゆれ一致採点 (`lenient_score.py`)
    LenientScore {
        #[arg(long)]
        dict: PathBuf,
        #[arg(long)]
        show: bool,
        paths: Vec<PathBuf>,
    },
    /// 採用条件判定 (`adoption_check.py`)
    AdoptionCheck {
        base: PathBuf,
        cand: PathBuf,
        split: String,
        #[arg(long)]
        dict: PathBuf,
    },
    /// オフライン評価結果前後比較 (`compare_quality.py`)
    CompareQuality {
        before: PathBuf,
        after: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// フォルダ間 1 問比較 (`compare_runs.py`)
    CompareRuns {
        before: PathBuf,
        after: PathBuf,
        #[arg(long, default_value = "")]
        before_prefix: String,
        #[arg(long, default_value = "")]
        after_prefix: String,
        names: Vec<String>,
    },
    /// 品質トリアージ失敗例抽出 (`quality_triage.py`)
    QualityTriage {
        #[arg(long)]
        before: PathBuf,
        #[arg(long)]
        after: PathBuf,
        #[arg(long)]
        baseline: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        require_ai: bool,
    },
    /// 同一失敗シナリオ抽出 (`make_same_mistake_set.py`)
    MakeSameMistakeSet {
        set: PathBuf,
        results: PathBuf,
        out: PathBuf,
    },
    /// 回帰ゴールデン管理 (`regression_add.py`)
    RegressionAdd {
        #[arg(long)]
        file: Option<PathBuf>,
        #[arg(long)]
        input: Option<String>,
        #[arg(long)]
        expected: Vec<String>,
        #[arg(long, default_value = "")]
        context: String,
        #[arg(long)]
        check: bool,
        #[arg(long)]
        list: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// 多タイミング回帰マトリクス実行 (`regression_matrix.py`)
    RegressionMatrix {
        exe: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        before: PathBuf,
        #[arg(long)]
        ajimee: Option<PathBuf>,
        #[arg(long)]
        only: Option<Vec<String>>,
        #[arg(long = "env")]
        envs: Vec<String>,
    },
    /// 実 IME ベンチ実行 (`eval/imebench/imebench.py`)
    ImeBench {
        ime: String,
        #[arg(long)]
        data: Vec<PathBuf>,
        #[arg(long)]
        n: Option<usize>,
        #[arg(long, default_value_t = 1200)]
        wait: u64,
        #[arg(long, default_value = "")]
        label: String,
    },
    /// `imebench` TSV → JSON 変換 (`eval/imebench/to_rows.py`)
    ImeBenchToRows {
        data: PathBuf,
        tsv: PathBuf,
        out: PathBuf,
    },
    /// 文節候補ダンプ (`eval/tools/dump_cands.py`)
    DumpCands {
        exe: PathBuf,
        data: PathBuf,
        out: PathBuf,
        #[arg(default_value_t = 8)]
        k: usize,
    },
    /// 起動時品質確認 (`check_startup_quality.py`)
    CheckStartupQuality {
        #[arg(long)]
        converter: PathBuf,
        #[arg(long)]
        model_dir: PathBuf,
        #[arg(long)]
        runtime_dir: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        reading: String,
        #[arg(long)]
        expected: String,
        #[arg(long, default_value_t = 60.0)]
        ready_timeout: f64,
    },
    /// 長期大量入力キャンペーン (`large_input_campaign.py`)
    LargeInputCampaign {
        #[arg(long)]
        exe: PathBuf,
        #[arg(long)]
        cases: PathBuf,
        #[arg(long)]
        install: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long, default_value_t = 10)]
        interaction_samples: usize,
        #[arg(long, default_value_t = 0.8)]
        settle_secs: f64,
        #[arg(long, default_value_t = 0.04)]
        key_interval_secs: f64,
        #[arg(long)]
        cpu: bool,
        #[arg(long, default_value_t = 15.0)]
        warm_secs: f64,
    },
    /// 大量入力ケース生成 (`large_input_cases.py`)
    LargeInputCases {
        #[arg(long)]
        mozc: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 150)]
        book_limit: usize,
        #[arg(long, default_value_t = 150)]
        name_limit: usize,
        #[arg(long, default_value_t = 20261102)]
        seed: u64,
    },
    /// 連続負荷ストレステスト (`stress_test.py`)
    StressTest {
        #[arg(long = "env")]
        envs: Vec<String>,
    },
    /// Space 変換遅延計測 (`space_latency.py`)
    SpaceLatency {
        #[arg(long, default_value_t = 0.05)]
        interval: f64,
        #[arg(long, default_value_t = 40)]
        n: usize,
        #[arg(long, default_value_t = 0.2)]
        pause: f64,
        #[arg(long)]
        data: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value_t = 2)]
        warm: usize,
        #[arg(long = "env")]
        envs: Vec<String>,
    },
    /// 打鍵遅延計測 (`keystroke_latency.py`)
    KeystrokeLatency {
        exe: PathBuf,
        #[arg(long)]
        data: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        models: Option<PathBuf>,
        #[arg(long = "config")]
        configs: Vec<String>,
        #[arg(long, default_value_t = 15.0)]
        warm: f64,
        #[arg(long, default_value_t = 30.0)]
        gap_ms: f64,
    },
    /// セッションランダムファズ試験 (`session_fuzz.py`)
    SessionFuzz {
        exe: PathBuf,
        #[arg(long)]
        models: Option<PathBuf>,
        #[arg(long, default_value_t = 30)]
        rounds: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },
    /// セッション CLI 受入れ (`session_cli_acceptance.py`)
    SessionCliAcceptance { exe: PathBuf },
    /// 確定再現リプレイ (`commit_replay.py`)
    CommitReplay {
        exe: PathBuf,
        #[arg(long)]
        data: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        models: Option<PathBuf>,
        #[arg(long, default_value_t = 20)]
        n: usize,
        #[arg(long, default_value_t = 12.0)]
        key_ms: f64,
        #[arg(long, default_value_t = 250.0)]
        pause_ms: f64,
        #[arg(long, default_value_t = 1500.0)]
        after_space_ms: f64,
    },
    /// QOD ログリプレイ (`qod_replay.py`)
    QodReplay {
        exe: PathBuf,
        models: PathBuf,
        log: PathBuf,
        #[arg(long = "config")]
        configs: Vec<String>,
    },
    /// 候補窓遅延プロファイル (`window_profile.py`)
    WindowProfile {
        exe: PathBuf,
        models: PathBuf,
        data: PathBuf,
        out: PathBuf,
        #[arg(long = "config")]
        configs: Vec<String>,
    },

    /// WiX UI BMP 素材とライセンス文生成 (`gen_assets.py`)
    GenAssets {
        out: PathBuf,
        #[arg(long)]
        images_only: bool,
    },
    /// Win32 ICO 群生成 (`gen_icons.py`)
    GenIcons {
        out: PathBuf,
        #[arg(long)]
        preview: Option<PathBuf>,
        #[arg(long)]
        brand_only: bool,
    },
    /// 折り紙 SVG アイコン PNG/ICO 出力 (`package_origami_icons.py`)
    PackageOrigamiIcons { out: PathBuf },
    /// README 図版生成 (`docs/images/gen_readme_images.py`)
    GenReadmeImages {
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
    /// 実入力比較図版生成 (`docs/images/gen_ime_comparison.py`)
    GenImeComparison {
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
    /// Win32 ウィンドウ撮影 (`capture_window.py`)
    CaptureWindow {
        exe: PathBuf,
        arg: String,
        out: PathBuf,
        #[arg(long, default_value_t = 5.0)]
        wait: f64,
        #[arg(long, default_value_t = 0)]
        tabs: usize,
        #[arg(long)]
        query: Option<String>,
    },
    /// PE アイコン資源タイル化 (`dump_icons.py`)
    DumpIcons { binary: PathBuf, out: PathBuf },
}

fn parse_kv(items: &[String]) -> Vec<(String, String)> {
    items
        .iter()
        .filter_map(|s| s.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

fn run(cli: Cli) -> Result<i32> {
    let repo = match cli.repo_root {
        Some(p) => p,
        None => {
            let cwd = std::env::current_dir()?;
            find_repo_root(&cwd).unwrap_or(cwd)
        }
    };
    match cli.cmd {
        Cmd::ReleaseTag {
            channel,
            repo_url,
            notes,
            json,
        } => {
            let rt = release_tag::run(&repo, &channel, &repo_url, notes.as_deref())?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({
                        "TAG": rt.tag,
                        "PRERELEASE": rt.prerelease,
                        "MAKE_LATEST": rt.make_latest,
                    })
                );
            } else {
                println!("TAG={}", rt.tag);
                println!(
                    "PRERELEASE={}",
                    if rt.prerelease { "true" } else { "false" }
                );
                println!(
                    "MAKE_LATEST={}",
                    if rt.make_latest { "true" } else { "false" }
                );
            }
            Ok(0)
        }
        Cmd::ReleaseName { tag, date } => {
            let parsed_date = match date.as_deref() {
                Some(s) => SimpleDate::parse(s)?,
                None => SimpleDate::today_jst(),
            };
            println!("{}", release_name(&tag, parsed_date)?);
            Ok(0)
        }
        Cmd::CheckRelease {
            tag,
            work,
            local,
            previous_local,
            require_generator,
            require_collocations,
            require_short_lexicon,
        } => {
            let opts = CheckReleaseOpts {
                tag,
                work,
                local,
                previous_local,
                require_generator,
                require_collocations,
                require_short_lexicon,
            };
            run_check_release(&opts, &repo)
        }
        Cmd::InstallerSmoke {
            msi,
            out,
            acceptance,
            duplicate_stress,
            browser_duplicate_stress,
            reinstall,
        } => {
            let opts = SmokeOpts {
                msi,
                out,
                acceptance,
                duplicate_stress,
                browser_duplicate_stress,
                reinstall,
            };
            run_installer_smoke(&opts, &repo)
        }
        Cmd::LocalInstallCheck {
            stage,
            out,
            candidate,
            rollback,
            expected_dir,
            restore_profile,
            skip_click,
            python,
            ui,
        } => {
            let opts = LocalInstallOpts {
                stage,
                out,
                candidate,
                rollback,
                expected_dir,
                restore_profile_flag: restore_profile,
                skip_click,
                python,
                ui,
            };
            run_local_install_check(&opts, &repo)?;
            Ok(0)
        }
        Cmd::InstalledProbe {
            out,
            wait_ms,
            live,
            keys,
        } => {
            let txt = run_installed_probe(&out, wait_ms, live, &keys, &repo)?;
            println!("{txt}");
            Ok(0)
        }
        Cmd::LiveConversionCheck { out, wait_ms } => {
            let v = run_live_conversion_check(&out, wait_ms, &repo)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::CollocationsStage {
            baseline,
            out,
            cache,
            ajimee,
            usage_dir,
            cforms,
        } => {
            let sha = collocations::run_collocations_stage(
                &repo,
                &baseline,
                &out,
                cache.as_deref(),
                ajimee.as_deref(),
                usage_dir.as_deref(),
                cforms.as_deref(),
            )?;
            println!("{sha}");
            Ok(0)
        }
        Cmd::BuildFastCollocations {
            corpus,
            corpus_manifest,
            baseline,
            exclude,
            out,
        } => {
            let v = collocations::run_build_fast_collocations(
                &corpus,
                &corpus_manifest,
                &baseline,
                &exclude,
                &out,
            )?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::BuildUsageCollocations {
            baseline,
            usage,
            cforms,
            out,
        } => {
            let v = collocations::run_build_usage_collocations(&baseline, &usage, &cforms, &out)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::LexiconFetchSources { out, cache, config } => {
            let v = lexicon::load_sources(&out, cache.as_deref(), &config)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::LexiconStagePackage {
            out,
            source,
            manifest,
        } => {
            let m = manifest.unwrap_or_else(|| repo.join("training/lexicon/package.json"));
            let v = lexicon::stage_package(&out, source.as_deref(), &m)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::LexiconStageTokenizer { out, cache } => {
            let v = lexicon::stage_tokenizer(&repo, &out, cache.as_deref())?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::LexiconBuild {
            source,
            mozc,
            out,
            config,
        } => {
            let cfg: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(config)?)?;
            let v = lexicon::build_short_lexicon(&source, &mozc, &out, &cfg, Some(&repo))?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::LexiconCollectFrequency {
            source,
            runtime_dir,
            out,
            exclude,
            shards,
            limit_bytes,
        } => {
            let v = frequency::collect_frequency(
                &repo,
                &source,
                &runtime_dir,
                &out,
                &exclude,
                shards,
                limit_bytes,
            )?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::LexiconBuildFrequency {
            candidate_dir,
            frequency_dir,
            mozc,
            out,
        } => {
            let v = lexicon::build_frequency_lexicon(
                &repo,
                &candidate_dir,
                &frequency_dir,
                &mozc,
                &out,
            )?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::CollocationsBuildFrequency {
            frequency_dir,
            baseline,
            runtime_path,
            out,
        } => {
            let v = lexicon::build_frequency_collocations(
                &repo,
                &frequency_dir,
                &baseline,
                &runtime_path,
                &out,
            )?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::LexiconBuildDiagnostic {
            candidate_dir,
            frequency_dir,
            mozc,
            exclude,
            out,
        } => {
            let v = lexicon::build_diagnostic(
                &repo,
                &candidate_dir,
                &frequency_dir,
                &mozc,
                &exclude,
                &out,
            )?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::FetchGenerator {
            out,
            source,
            manifest,
        } => {
            let p = training_prep::fetch_generator(
                &repo,
                &out,
                source.as_deref(),
                manifest.as_deref(),
            )?;
            println!("{}", p.display());
            Ok(0)
        }
        Cmd::StageGenerator {
            generator,
            base_dir,
            out,
        } => {
            let v = training_prep::stage_generator(&repo, &generator, &base_dir, &out)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::NativePairs {
            observed,
            general,
            out,
        } => {
            let v = training_prep::run_native_pairs(&observed, &general, &out, &repo)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::BuildEvalSets {
            sources,
            out,
            dict,
            judge_per_domain,
            dev_per_domain,
        } => {
            let v = eval_sets::run_build_eval_sets(
                &sources,
                &out,
                &dict,
                judge_per_domain,
                dev_per_domain,
            )?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::BuildContextChoice {
            sources,
            out,
            dict,
            judge_pairs,
            dev_pairs,
            judge_passages,
            dev_passages,
            passage_len,
        } => {
            let v = eval_sets::run_build_context_choice(
                &sources,
                &out,
                &dict,
                judge_pairs,
                dev_pairs,
                judge_passages,
                dev_passages,
                passage_len,
            )?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::BuildContextPairs {
            sources,
            out,
            dict,
            judge_pairs,
            dev_pairs,
        } => {
            let v =
                eval_sets::run_build_context_pairs(&sources, &out, &dict, judge_pairs, dev_pairs)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::BuildKanaPreference {
            sources,
            out,
            dict,
            min_count,
        } => {
            let v = eval_sets::run_build_kana_preference(&sources, &out, &dict, min_count)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::BuildNameLexicon {
            articles,
            exclude,
            mozc_dict,
            short_lexicon,
            out,
            dict,
            cost_offset,
        } => {
            let ex_refs: Vec<&Path> = exclude.iter().map(PathBuf::as_path).collect();
            let v = eval_sets::run_build_name_lexicon(
                &articles,
                &ex_refs,
                &mozc_dict,
                &short_lexicon,
                &out,
                &dict,
                cost_offset,
            )?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::ImportSonnetChoice { src, out, dict } => {
            let v = eval_sets::run_import_sonnet_choice(&src, &out, &dict)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::FetchWikipediaNewArticles {
            out,
            start,
            end,
            per_month,
        } => {
            let v = eval_sets::fetch_wikipedia_new_articles(&out, &start, &end, per_month)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::GenNotationTable { tsv, meta, out } => {
            let n = eval_sets::gen_notation_table(&tsv, &meta, &out)?;
            println!("{n}");
            Ok(0)
        }
        Cmd::GenContextDefaults { json, out } => {
            let v = eval_sets::gen_context_defaults_inc(&json, &out)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::MakeAtokSets { out } => {
            eval_sets::run_make_atok_sets(&out)?;
            Ok(0)
        }
        Cmd::RankerPrepare {
            corpus,
            out,
            exclude,
            source_commit,
            train_limit,
        } => {
            let ex_refs: Vec<&Path> = exclude.iter().map(PathBuf::as_path).collect();
            let v =
                initial_ranker::run_prepare(&corpus, &out, &ex_refs, &source_commit, train_limit)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::RankerCollect {
            exe,
            data,
            out,
            source_commit,
        } => {
            let v = initial_ranker::collect_lattice(&exe, &data, &out, &source_commit)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::RankerCalibrate {
            observations,
            model,
            out,
        } => {
            let v = initial_ranker::run_calibrate(&observations, &model, &out)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::RankerEvaluate {
            observations,
            model,
            weight,
            margin,
            out,
        } => {
            let v = initial_ranker::run_evaluate(&observations, &model, weight, margin, &out)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::EvalBaseline {
            converter_main,
            data,
            out,
            profile,
            context,
            timeout,
            stderr_out,
            min_acc,
            converter_source_commit,
            case_traces,
            request_traces,
        } => {
            let opts = baseline::EvalBaselineOpts {
                converter_main,
                data,
                out,
                profile,
                context,
                timeout_secs: timeout,
                stderr_out,
                min_acc,
                converter_source_commit,
                case_traces,
                request_traces,
            };
            baseline::run_eval_baseline(&opts, &repo)
        }
        Cmd::EvalPredict {
            converter_main,
            data,
            typed_len,
            top_k,
            min_len,
            out,
        } => {
            baseline::run_eval_predict(
                &converter_main,
                &data,
                typed_len,
                top_k,
                min_len,
                out.as_deref(),
            )?;
            Ok(0)
        }
        Cmd::SessionEval {
            exe,
            data,
            out,
            context,
            models,
            ai_state,
            warm,
            limit,
            immediate_space,
            space_gap,
            live,
            timeline,
            configs,
            envs,
        } => {
            let opts = baseline::SessionEvalOpts {
                exe,
                data,
                out,
                context,
                models,
                ai_state,
                warm,
                limit,
                config: parse_kv(&configs),
                immediate_space,
                space_gap_ms: space_gap,
                live,
                timeline,
                extra_env: parse_kv(&envs),
            };
            baseline::run_session_eval(&opts, &repo)?;
            Ok(0)
        }
        Cmd::LenientScore { dict, show, paths } => {
            let refs: Vec<&Path> = paths.iter().map(PathBuf::as_path).collect();
            let _ = tools::run_lenient_score(&refs, &dict, show)?;
            Ok(0)
        }
        Cmd::AdoptionCheck {
            base,
            cand,
            split,
            dict,
        } => {
            let v = tools::run_adoption_check(&base, &cand, &split, &dict)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::CompareQuality { before, after, out } => {
            let v = tools::run_compare_quality(&before, &after, &out)?;
            println!(
                "改善 {} / 改悪 {} / 変更 {} / 全 {}",
                v["improved"], v["regressed"], v["changed"], v["items"]
            );
            Ok(0)
        }
        Cmd::CompareRuns {
            before,
            after,
            before_prefix,
            after_prefix,
            names,
        } => {
            let _ =
                tools::run_compare_runs(&before, &after, &names, &before_prefix, &after_prefix)?;
            Ok(0)
        }
        Cmd::QualityTriage {
            before,
            after,
            baseline,
            out,
            require_ai,
        } => {
            let _ = tools::run_quality_triage(&before, &after, &baseline, &out, require_ai, &repo)?;
            Ok(0)
        }
        Cmd::MakeSameMistakeSet { set, results, out } => {
            let n = tools::run_make_same_mistake_set(&set, &results, &out)?;
            println!("{n} 場面");
            Ok(0)
        }
        Cmd::RegressionAdd {
            file,
            input,
            expected,
            context,
            check,
            list,
            dry_run,
        } => {
            let f = file.unwrap_or_else(|| repo.join("eval/sets/asteria-regression-golden.json"));
            tools::run_regression_add(
                &f,
                input.as_deref(),
                &expected,
                &context,
                check,
                list,
                dry_run,
            )
        }
        Cmd::RegressionMatrix {
            exe,
            out,
            before,
            ajimee,
            only,
            envs,
        } => {
            tools::run_regression_matrix(
                &exe,
                &out,
                &before,
                ajimee.as_deref(),
                only.as_deref(),
                &parse_kv(&envs),
                &repo,
            )?;
            Ok(0)
        }
        Cmd::ImeBench {
            ime,
            data,
            n,
            wait,
            label,
        } => {
            let refs: Vec<&Path> = data.iter().map(PathBuf::as_path).collect();
            tools::run_imebench(&ime, &refs, n, wait, &label, &repo)?;
            Ok(0)
        }
        Cmd::ImeBenchToRows { data, tsv, out } => {
            let n = tools::run_imebench_to_rows(&data, &tsv, &out)?;
            println!("{n} rows");
            Ok(0)
        }
        Cmd::DumpCands { exe, data, out, k } => {
            let n = tools::run_dump_cands(&exe, &data, &out, k)?;
            println!("{n} items");
            Ok(0)
        }
        Cmd::CheckStartupQuality {
            converter,
            model_dir,
            runtime_dir,
            out,
            reading,
            expected,
            ready_timeout,
        } => {
            let v = tools::run_check_startup_quality(
                &converter,
                &model_dir,
                &runtime_dir,
                &out,
                &reading,
                &expected,
                ready_timeout,
            )?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::LargeInputCampaign {
            exe,
            cases,
            install,
            out,
            limit,
            interaction_samples,
            settle_secs,
            key_interval_secs,
            cpu,
            warm_secs,
        } => {
            large_input::run_campaign(
                &exe,
                &cases,
                install.as_deref(),
                &out,
                limit,
                interaction_samples,
                settle_secs,
                key_interval_secs,
                cpu,
                warm_secs,
            )?;
            Ok(0)
        }
        Cmd::LargeInputCases {
            mozc,
            out,
            book_limit,
            name_limit,
            seed,
        } => {
            let (cases, manifest) =
                large_input::generate_cases(&mozc, &repo, book_limit, name_limit, seed)?;
            let mut body = String::new();
            for c in &cases {
                body.push_str(&serde_json::to_string(c)?);
                body.push('\n');
            }
            if let Some(p) = out.parent() {
                std::fs::create_dir_all(p)?;
            }
            std::fs::write(&out, body.as_bytes())?;
            std::fs::write(
                out.with_extension("manifest.json"),
                serde_json::to_string_pretty(&manifest)?,
            )?;
            println!("{} cases", cases.len());
            Ok(0)
        }
        Cmd::StressTest { envs } => {
            bench::run_stress_test(&parse_kv(&envs))?;
            Ok(0)
        }
        Cmd::SpaceLatency {
            interval,
            n,
            pause,
            data,
            out,
            warm,
            envs,
        } => {
            bench::run_space_latency(
                interval,
                n,
                pause,
                &data,
                out.as_deref(),
                warm,
                &parse_kv(&envs),
            )?;
            Ok(0)
        }
        Cmd::KeystrokeLatency {
            exe,
            data,
            out,
            models,
            configs,
            warm,
            gap_ms,
        } => {
            let v = bench::run_keystroke_latency(
                &exe,
                &data,
                &out,
                models.as_deref(),
                &parse_kv(&configs),
                warm,
                gap_ms,
                &repo,
            )?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::SessionFuzz {
            exe,
            models,
            rounds,
            seed,
        } => {
            let v = acceptance::run_session_fuzz(&exe, models.as_deref(), rounds, seed, &repo)?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::SessionCliAcceptance { exe } => {
            acceptance::run_session_cli_acceptance(&exe)?;
            Ok(0)
        }
        Cmd::CommitReplay {
            exe,
            data,
            out,
            models,
            n,
            key_ms,
            pause_ms,
            after_space_ms,
        } => {
            let v = tools::run_commit_replay(
                &exe,
                &data,
                &out,
                models.as_deref(),
                n,
                key_ms,
                pause_ms,
                after_space_ms,
                &repo,
            )?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::QodReplay {
            exe,
            models,
            log,
            configs,
        } => {
            let steps = tools::run_qod_replay(&exe, &models, &log, &parse_kv(&configs))?;
            println!("OK steps={steps}");
            Ok(0)
        }
        Cmd::WindowProfile {
            exe,
            models,
            data,
            out,
            configs,
        } => {
            let v = tools::run_window_profile(&exe, &models, &data, &out, &parse_kv(&configs))?;
            println!("{}", serde_json::to_string_pretty(&v)?);
            Ok(0)
        }
        Cmd::GenAssets { out, images_only } => {
            installer::run_gen_assets(&out, images_only, &repo)?;
            Ok(0)
        }
        Cmd::GenIcons {
            out,
            preview,
            brand_only,
        } => {
            let n = icons::run_gen_icons(&out, preview.as_deref(), brand_only, &repo)?;
            println!("ok {n}");
            Ok(0)
        }
        Cmd::PackageOrigamiIcons { out } => {
            icons::run_package_origami_icons(&out)?;
            Ok(0)
        }
        Cmd::GenReadmeImages { out_dir } => {
            figures::run_gen_readme_images(out_dir.as_deref(), &repo)?;
            Ok(0)
        }
        Cmd::GenImeComparison { out_dir } => {
            figures::run_gen_ime_comparison(out_dir.as_deref(), &repo)?;
            Ok(0)
        }
        Cmd::CaptureWindow {
            exe,
            arg,
            out,
            wait,
            tabs,
            query,
        } => {
            capture::run_capture_window(&exe, Some(&arg), &out, wait, tabs, query.as_deref())?;
            Ok(0)
        }
        Cmd::DumpIcons { binary, out } => {
            let n = capture::run_dump_icons(&binary, &out)?;
            println!("{n} icons");
            Ok(0)
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(0) => ExitCode::SUCCESS,
        Ok(code) => ExitCode::from(code as u8),
        Err(err) => {
            eprintln!("{err:#}");
            ExitCode::from(1)
        }
    }
}
