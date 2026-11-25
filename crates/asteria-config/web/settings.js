// Asteria の設定画面(docs/adr/0084)。
// 設定は変えたらすぐ保存する(元に戻すときは同じ所で戻す)。保存・読み込みは WebView2 の
// メッセージで mozc_tool の AsteriaSettingsBridge に頼む。WebView2 の外(ブラウザーでの確認)では
// 見本のデータで動く。
'use strict';

(() => {
  // ---------------------------------------------------------------- 部品
  const $ = (selector, root = document) => root.querySelector(selector);

  // 要素を作る。attrs の on〜 は出来事、class・text・html は中身、それ以外は属性。
  function h(tag, attrs = {}, ...children) {
    const el = document.createElement(tag);
    for (const [key, value] of Object.entries(attrs)) {
      if (value === undefined || value === null || value === false) continue;
      if (key === 'class') el.className = value;
      else if (key === 'text') el.textContent = value;
      else if (key === 'html') el.innerHTML = value;
      else if (key.startsWith('on')) el.addEventListener(key.slice(2), value);
      else el.setAttribute(key, value === true ? '' : value);
    }
    for (const child of children.flat()) {
      if (child === null || child === undefined || child === false) continue;
      el.append(child instanceof Node ? child : document.createTextNode(child));
    }
    return el;
  }

  let uid = 0;
  const newId = (prefix) => `${prefix}-${++uid}`;

  const ICONS = {
    home: '<path d="M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z"/>',
    ai: '<path d="M12 2.5l1.9 4.6 4.6 1.9-4.6 1.9L12 15.5l-1.9-4.6L5.5 9l4.6-1.9zM18.5 14l1 2.5 2.5 1-2.5 1-1 2.5-1-2.5-2.5-1 2.5-1z"/>',
    input: '<rect x="2.5" y="6" width="19" height="12" rx="2"/><path d="M6 10h1M9 10h1M12 10h1M15 10h1M18 10h0M7 14h10"/>',
    convert: '<path d="M4 5h9M8.5 3v2M6 5c0 4 3 7 7 8M11 5c-1 4-4 7-7 8M14 21l4-10 4 10M15.5 17.5h5"/>',
    assist: '<path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M5.6 18.4l2.1-2.1M16.3 7.7l2.1-2.1"/><circle cx="12" cy="12" r="3"/>',
    suggest: '<path d="M4 6h16M4 12h10M4 18h6"/><path d="M17 15l3 3-3 3"/>',
    privacy: '<path d="M12 3l8 3v6c0 4.5-3.4 8.3-8 9-4.6-.7-8-4.5-8-9V6z"/><path d="M9 12l2 2 4-4"/>',
    other: '<circle cx="5" cy="12" r="1.6"/><circle cx="12" cy="12" r="1.6"/><circle cx="19" cy="12" r="1.6"/>',
    check: '<path d="M4 12.5l5 5L20 6.5"/>',
    info: '<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7.5v.5"/>',
    shield: '<path d="M12 3l8 3v6c0 4.5-3.4 8.3-8 9-4.6-.7-8-4.5-8-9V6z"/>',
  };

  function icon(name, cls) {
    const filled = name === 'other';
    return h('span', {
      'aria-hidden': 'true',
      html: `<svg viewBox="0 0 24 24" class="${cls || ''}" fill="${filled ? 'currentColor' : 'none'}" ` +
        `stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">${ICONS[name]}</svg>`,
    }).firstChild;
  }

  // ---------------------------------------------------------------- 橋渡し
  const native = window.chrome && window.chrome.webview;
  const pending = new Map();
  let nextRequest = 1;

  if (native) {
    native.addEventListener('message', (event) => {
      const { id, result, event: pushed } = event.data || {};
      if (pushed) {
        onPushed(pushed);
        return;
      }
      const resolve = pending.get(id);
      if (resolve) {
        pending.delete(id);
        resolve(result || { ok: false, message: '答えが空でした。' });
      }
    });
  }

  function call(op, args = {}) {
    if (!native) return Preview.call(op, args);
    return new Promise((resolve) => {
      const id = nextRequest++;
      pending.set(id, resolve);
      // 要求の番号と操作の名前は、引数で上書きしない(返事をこの番号で待つ)。
      native.postMessage(JSON.stringify({ ...args, id, op }));
    });
  }

  // ---------------------------------------------------------------- 項目の表
  const ON_OFF = (v) => (v ? 'オン' : 'オフ');
  const cfg = () => state.config;

  const QUALITY_NOTES = [
    'GPU のメモリと電気を節約します。小さい AI だけで動きます。',
    'おすすめ。いちばん正確な設定です。',
    '追加の AI で、さらに正確に採点します。',
    'いちばん大きい追加の AI で採点します。',
  ];
  const TIER_OF = { high: 2, unreal: 3 };
  // その段階の追加の AI(入っていなければ選べない)。
  const modelForTier = (v) => (state.models || []).find((m) => TIER_OF[m.tier] === v);

  const PAGES = [
    {
      id: 'home', title: 'ホーム', icon: 'home', render: renderHome,
      search: 'AI の状態 今日の使い方 変換した回数 正答率 メモリ 応援',
    },
    {
      id: 'ai', title: 'AI 変換', icon: 'ai',
      groups: [
        {
          title: '変換', items: [
            { type: 'switch', key: 'asteria_lm_enabled', label: 'AI 変換を使う', desc: 'オフでは Mozc と同じ変換になります。' },
            {
              type: 'segmented', key: 'asteria_lm_quality', label: '変換の品質',
              options: [[0, 'Eco'], [1, 'Standard'], [2, 'High'], [3, 'Unreal']],
              // High・Unreal は、追加の AI があるときだけ出し、入っているときだけ選べる。
              filter: ([v]) => v < 2 || modelForTier(v) !== undefined,
              optionDisabled: (v) => v >= 2 && !(modelForTier(v) || {}).installed,
              note: (v) => {
                const m = modelForTier(v);
                if (m && !m.installed) return `${QUALITY_NOTES[v]}下の「追加の AI」から ${m.title} を入れると選べます。`;
                return QUALITY_NOTES[v] || '';
              },
              recommended: 1, fallback: 1,
              disabledWhen: () => !cfg().asteria_lm_enabled,
            },
            { type: 'live', id: 'ai-status', label: 'この PC', desc: () => gpuChips().join(' ・ ') },
          ],
        },
        {
          title: '追加の AI', when: () => (state.models || []).length > 0, items: [
            { type: 'models', label: '追加の AI', search: 'ダウンロード High Unreal Swallow 判定役 追加 モデル' },
          ],
        },
        {
          title: '入力中の動き', items: [
            {
              type: 'switch', key: 'asteria_live_conversion', label: 'ライブ変換',
              desc: '手が止まると、変換した文字に入れ替わります。',
              example: 'きょうはいいてんき → 今日はいい天気', disabledWhen: () => !cfg().asteria_lm_enabled,
            },
            {
              type: 'switch', key: 'asteria_lm_correction', label: '入力中に誤変換を補正する',
              desc: '数字・名前・登録した語は変えません。',
              disabledWhen: () => !cfg().asteria_lm_enabled,
            },
            {
              type: 'switch', key: 'asteria_homophone_hints', label: '同音語のヒント',
              desc: '同じ読みの語に、見分ける手がかりを添えます。',
              example: '足 ― 靴・靴下 / 脚 ― 椅子・机',
            },
            {
              type: 'switch', key: 'asteria_romaji_english', label: '英語の名前をローマ字のまま打つ',
              desc: '文に合えば英語の名前にします。',
              example: 'ubuntuwoirete → Ubuntuを入れて',
            },
          ],
        },
        {
          title: 'GPU', items: [
            {
              type: 'switch', key: 'asteria_use_integrated_gpu', label: '内蔵 GPU で AI を動かす', tags: [['試験的', 'lab']],
              desc: '専用の GPU がない PC 向け。再起動で反映します。',
            },
          ],
        },
        {
          title: '詳しい設定', items: [
            {
              type: 'select', key: 'asteria_lm_model', label: 'AI モデル',
              options: [[0, '自動(おすすめ)'], [1, 'zenz のみ(軽い)'], [2, 'zenz + LLM(GPU 向け)']],
            },
            {
              type: 'text', key: 'asteria_lm_model_path', label: 'LLM のファイル',
              desc: 'GGUF のファイル。空なら同梱のモデル。',
              placeholder: '同梱のモデルを使う', browse: true,
            },
            {
              type: 'button', label: '診断情報', button: '診断情報を見る…', action: showDiagnostics,
              desc: '不具合の報告用。入力した文字は含みません。',
            },
          ],
        },
      ],
    },
    {
      id: 'input', title: '入力', icon: 'input',
      groups: [
        {
          title: '基本', items: [
            {
              type: 'select', label: '入力のしかた',
              options: [[0, 'ローマ字'], [1, 'かな'], [2, 'ローマ字(キーで切り替え)'], [3, 'かな(キーで切り替え)']],
              get: () => cfg().preedit_method + (cfg().use_keyboard_to_change_preedit_method ? 2 : 0),
              set: (v) => {
                cfg().preedit_method = v % 2;
                cfg().use_keyboard_to_change_preedit_method = v >= 2;
              },
              search: 'ローマ字入力 かな入力',
            },
            {
              type: 'select', key: 'punctuation_method', label: '句読点',
              options: [[0, '、。'], [1, '，．'], [2, '、．'], [3, '，。']],
            },
            {
              type: 'select', key: 'symbol_method', label: 'かぎかっこ・中黒',
              options: [[0, '「」・'], [1, '[]／'], [2, '「」／'], [3, '[]・']],
            },
            {
              type: 'select', key: 'space_character_form', label: '空白の幅',
              options: [[0, '入力モードに従う'], [1, 'いつも全角'], [2, 'いつも半角']],
            },
            {
              type: 'select', key: 'selection_shortcut', label: '数字キーで候補を選ぶ',
              options: [[0, '使わない'], [1, '1〜9'], [2, 'A〜L']],
            },
            {
              type: 'select', key: 'numpad_character_form', label: 'テンキーの数字',
              options: [[0, '入力モードに従う'], [1, '全角'], [2, '半角'], [3, '直接入力']],
            },
          ],
        },
        {
          title: 'キー操作', items: [
            {
              type: 'select', key: 'session_keymap', label: 'キーの割り当て',
              options: [[-1, '既定(MS-IME と同じ)'], [2, 'MS-IME'], [1, 'ATOK'], [3, 'ことえり'], [0, '自分で変えた割り当て']],
              filter: ([v]) => v !== 0 || state.env.keymapCustomized || cfg().session_keymap === 0,
            },
            {
              type: 'button', label: 'キーを 1 つずつ変える', button: 'カスタマイズ…', action: () => openNative('keymap'),
            },
            {
              type: 'button', label: 'ローマ字の表', button: 'カスタマイズ…', action: () => openNative('romanTable'),
              desc: () => (state.env.romanTableCustomized ? '自分で変えた表を使っています。' : ''),
              disabledWhen: () => cfg().preedit_method !== 0,
            },
          ],
        },
      ],
    },
    {
      id: 'convert', title: '変換と辞書', icon: 'convert',
      groups: [
        {
          title: '学習', items: [
            {
              type: 'select', key: 'history_learning_level', label: '選んだ言葉を覚える',
              desc: '選び直した文も、次から同じ文で出します。',
              options: [[0, '覚える'], [1, '覚えた分だけ使う(新しく覚えない)'], [2, '覚えない']],
            },
            {
              type: 'button', label: '覚えた言葉を消す', button: '消す…', danger: true,
              action: () => clear('history', '覚えた言葉を消しますか?', '変換で覚えた言葉と、選び直した文をすべて消します。元には戻せません。'),
            },
          ],
        },
        {
          title: 'ユーザー辞書', items: [
            {
              type: 'button', label: 'ユーザー辞書', button: '開く…', action: () => openNative('dictionary'),
            },
          ],
        },
        {
          title: '特別な変換', items: [
            { type: 'switch', key: 'use_single_kanji_conversion', label: '漢字 1 字', example: 'か → 火・課・蚊' },
            { type: 'switch', key: 'use_symbol_conversion', label: '記号', example: 'やじるし → →・⇒' },
            { type: 'switch', key: 'use_emoticon_conversion', label: '顔文字', example: 'かおもじ → (^_^)' },
            { type: 'switch', key: 'use_emoji_conversion', label: '絵文字', example: 'はーと → ❤' },
            { type: 'switch', key: 'use_t13n_conversion', label: 'カタカナ語を英語に', example: 'こんぴゅーたー → computer' },
            { type: 'switch', key: 'use_zip_code_conversion', label: '郵便番号から住所', example: '100-0001 → 東京都千代田区千代田', desc: 'ハイフンを入れて打ちます。' },
            { type: 'switch', key: 'use_date_conversion', label: '日付と時刻', example: 'きょう → 今日の日付' },
            { type: 'switch', key: 'use_number_conversion', label: '数字の書き方', example: '12 → 十二・Ⅻ' },
            { type: 'switch', key: 'use_calculator', label: '計算', example: '1+2= → 3' },
            { type: 'switch', key: 'use_spelling_correction', label: '読みの覚え違いを候補に出す' },
            { type: 'switch', key: 'use_local_usage_dictionary', label: '言葉の使い分けを候補の横に出す' },
          ],
        },
      ],
    },
    {
      id: 'assist', title: '入力の補助', icon: 'assist',
      groups: [
        {
          title: '入力の補助', items: [
            {
              type: 'switch', key: 'auto_switch_composition_mode', label: '英数字を自動で半角にする',
            },
            {
              type: 'switch', key: 'use_auto_conversion', label: '句読点を打ったら変換する',
              desc: '下の記号を打つと、そこまでを変換します。',
            },
            {
              type: 'checks', key: 'auto_conversion_key', label: '変換する記号',
              bits: [[1, '。', '句点'], [2, '、', '読点'], [4, '?', '疑問符'], [8, '!', '感嘆符']],
              disabledWhen: () => !cfg().use_auto_conversion,
            },
            {
              type: 'select', key: 'shift_key_mode_switch', label: 'Shift を押しながら英字を打ったとき',
              options: [[0, '切り替えない'], [1, '英数字にする'], [2, 'カタカナにする']],
            },
            {
              type: 'switch', key: 'use_mode_indicator', label: '入力モード(あ・A)をカーソルの近くに出す',
            },
            {
              type: 'switch', key: 'asteria_share_input_mode', label: '入力モードをすべてのアプリで共有する',
            },
          ],
        },
        {
          title: '全角・半角', items: [
            { type: 'forms', label: '文字の形', search: '全角 半角 カタカナ 数字 英字 記号 前回と同じ' },
          ],
        },
      ],
    },
    {
      id: 'suggest', title: '予測', icon: 'suggest',
      lead: '続きになる言葉があるときだけ窓に出します。Space の変換の結果は変わりません。',
      groups: [
        {
          title: '予測の元', items: [
            { type: 'switch', key: 'use_history_suggest', label: '入力履歴から予測する' },
            { type: 'switch', key: 'use_dictionary_suggest', label: 'システム辞書から予測する' },
          ],
        },
        {
          title: '出し方', items: [
            { type: 'stepper', key: 'suggestions_size', label: '予測の候補数', min: 1, max: 9, unit: '件' },
            {
              type: 'stepper', key: 'asteria_suggest_min_chars', label: '予測を出し始める文字数', min: 1, max: 5, unit: '文字',
            },
          ],
        },
        {
          title: '入力履歴', items: [
            {
              type: 'button', label: '使っていない履歴を消す', button: '消す…',
              action: () => clear('unusedPrediction', '使っていない履歴を消しますか?', 'しばらく使っていない入力履歴を消します。'),
            },
            {
              type: 'button', label: '入力履歴をすべて消す', button: 'すべて消す…', danger: true,
              action: () => clear('prediction', '入力履歴をすべて消しますか?', '予測に使う入力履歴をすべて消します。元には戻せません。'),
            },
          ],
        },
      ],
    },
    {
      id: 'privacy', title: 'プライバシー', icon: 'privacy',
      lead: '入力した文字・覚えた言葉・使い方は外に送りません。AI もこの PC の中だけで動きます。',
      groups: [
        {
          title: '一時的に止める', items: [
            {
              type: 'switch', key: 'incognito_mode', label: 'シークレットモード',
              desc: '学習・履歴・ユーザー辞書を止めます。',
            },
            {
              type: 'switch', key: 'presentation_mode', label: 'プレゼンテーションモード',
              desc: '入力中の予測の窓を出しません。',
              disabledWhen: () => !cfg().use_history_suggest && !cfg().use_dictionary_suggest,
            },
          ],
        },
        {
          title: '誤変換の報告',
          items: [
            {
              type: 'switch', key: 'asteria_recent_conversions', label: '直近の変換をこの PC に残す',
              desc: '誤変換を見つけたときに、読み・前の文・候補・確定した語をあとから確かめられます。外へは送りません。',
            },
            {
              type: 'recent', label: '直近の変換',
              search: '誤変換 報告 直近 候補 読み 前の文 書き出す 消す',
              desc: '確認したいときにだけ、この PC の記録を読みます。',
            },
          ],
        },
      ],
    },
    {
      id: 'other', title: 'その他', icon: 'other',
      groups: [
        {
          title: 'Windows', items: [
            { type: 'switch', key: 'check_default', label: '起動したときに Asteria が既定の IME か確かめる' },
            { type: 'env', key: 'imeHotKeyDisabled', label: 'Ctrl+Shift でキーボードを切り替えない' },
            {
              type: 'button', label: '辞書の先読みと管理者の設定', button: '開く…', shield: true, action: () => openNative('administration'),
              desc: '管理者の権限が要ります。',
            },
          ],
        },
        {
          title: 'お知らせ', items: [
            { type: 'env', key: 'supportReminder', label: 'GitHub での応援のお願いを通知で表示する', desc: '3 日に 1 回です。' },
          ],
        },
        {
          title: '初期設定', items: [
            {
              type: 'button', label: '設定を初期設定に戻す', button: '戻す…', danger: true, action: resetDefaults,
              desc: '覚えた言葉と辞書は残ります。',
            },
          ],
        },
      ],
    },
  ];

  // ---------------------------------------------------------------- 状態
  const state = {
    config: {}, env: {}, page: 'home', query: '', status: null,
    models: [], downloads: {}, gpuVramGb: 0,
  };
  const rows = [];  // 今の画面の行(状態が変わったら見直す)

  // ---------------------------------------------------------------- 保存
  let saveTimer = 0;
  let saving = Promise.resolve();
  let lastSavedJson = '';
  let pendingEnv = {};

  function scheduleSave(env) {
    if (env) Object.assign(pendingEnv, env);
    clearTimeout(saveTimer);
    showSave('saving');
    saveTimer = setTimeout(save, 260);
  }

  function save() {
    const env = pendingEnv;
    pendingEnv = {};
    const config = JSON.parse(JSON.stringify(state.config));
    saving = saving.then(async () => {
      const result = await call('save', { config, env });
      if (!result.ok) {
        showSave('error');
        toast(result.message || '保存できませんでした。', 'bad');
        return;
      }
      lastSavedJson = JSON.stringify(result.config);
      if (JSON.stringify(state.config) === JSON.stringify(config)) state.config = result.config;
      state.env = result.env || state.env;
      showSave('saved');
      for (const notice of result.notices || []) toast(notice);
      refreshRows();
    });
  }

  let saveHide = 0;
  function showSave(kind) {
    const el = $('#save-state');
    clearTimeout(saveHide);
    el.className = `save-state show ${kind}`;
    el.replaceChildren();
    if (kind === 'saving') {
      el.append(h('span', { class: 'spinner', 'aria-hidden': 'true' }), '保存しています');
    } else if (kind === 'saved') {
      el.append(icon('check'), '保存しました');
      saveHide = setTimeout(() => el.classList.remove('show'), 2200);
    } else {
      el.append(icon('info'), '保存できませんでした ― ', h('button', {
        class: 'btn', text: 'もう一度', onclick: () => scheduleSave(),
      }));
    }
  }

  function toast(text, kind = '') {
    const el = h('div', { class: `toast ${kind}`, role: kind === 'bad' ? 'alert' : 'status' }, icon('info'), h('div', { text }));
    $('#toasts').append(el);
    setTimeout(() => {
      el.classList.add('out');
      el.addEventListener('animationend', () => el.remove(), { once: true });
    }, kind === 'bad' ? 7000 : 4500);
  }

  // ---------------------------------------------------------------- 確かめる窓
  function confirmDialog(title, text, ok, danger) {
    const dialog = $('#confirm');
    $('#confirm-title').textContent = title;
    $('#confirm-text').textContent = text;
    const okButton = $('#confirm-ok');
    okButton.textContent = ok;
    okButton.className = `btn primary${danger ? ' danger-primary' : ''}`;
    return new Promise((resolve) => {
      const done = (value) => {
        dialog.close();
        okButton.onclick = null;
        $('#confirm-cancel').onclick = null;
        resolve(value);
      };
      okButton.onclick = () => done(true);
      $('#confirm-cancel').onclick = () => done(false);
      dialog.oncancel = (e) => { e.preventDefault(); done(false); };
      dialog.showModal();
      $('#confirm-cancel').focus();
    });
  }

  async function clear(what, title, text) {
    if (!await confirmDialog(title, text, '消す', true)) return;
    const result = await call('clear', { what });
    toast(result.ok ? '消しました。' : result.message, result.ok ? '' : 'bad');
  }

  async function openNative(target) {
    const result = await call('open', { target });
    if (!result.ok) {
      toast(result.message, 'bad');
      return;
    }
    if (result.changed && result.config) {
      state.config = result.config;
      state.env = result.env || state.env;
      render();
      showSave('saved');
    }
  }

  async function showDiagnostics() {
    const result = await call('diagnostics');
    if (!result.ok) {
      toast(result.message, 'bad');
      return;
    }
    const dialog = $('#diagnostics');
    $('#diag-text').value = result.text;
    $('#diag-close').onclick = () => dialog.close();
    $('#diag-save').onclick = async () => {
      const saved = await call('saveDiagnostics');
      if (saved.ok && saved.saved) {
        dialog.close();
        toast('診断情報を保存しました。');
      }
    };
    dialog.showModal();
  }

  async function resetDefaults() {
    if (!await confirmDialog('設定を初期設定に戻しますか?',
      'すべての設定を最初の状態に戻します。覚えた言葉・入力履歴・ユーザー辞書・管理者の設定はそのまま残ります。',
      '初期設定に戻す', true)) return;
    const result = await call('defaults');
    if (!result.ok) return;
    state.config = result.config;
    render();
    scheduleSave();
    toast('初期設定に戻しました。');
  }

  // ---------------------------------------------------------------- 行の部品
  const text = (value) => (typeof value === 'function' ? value() : value);

  function rowShell(item, control, extra) {
    const labelId = newId('label');
    const descId = newId('desc');
    const desc = text(item.desc);
    const row = h('div', { class: 'row', 'data-search': searchText(item) },
      h('div', { class: 'row-text' },
        h('div', { class: 'row-label', id: labelId },
          highlight(item.label),
          (item.tags || []).map(([t, c]) => h('span', { class: `tag ${c || ''}`, text: t }))),
        desc ? h('div', { class: 'row-desc', id: descId }, highlight(desc)) : null,
        item.example ? h('span', { class: 'row-example' }, highlight(item.example)) : null,
        extra || null),
      control ? h('div', { class: 'control' }, control) : null);
    row.labelId = labelId;
    row.descId = desc ? descId : null;
    return row;
  }

  function buildSwitch(item, get, set) {
    const input = h('input', { type: 'checkbox', role: 'switch' });
    const stateText = h('span', { class: 'state', 'aria-hidden': 'true' });
    const label = h('label', { class: 'switch' }, stateText, input);
    const row = rowShell(item, label);
    input.setAttribute('aria-labelledby', row.labelId);
    if (row.descId) input.setAttribute('aria-describedby', row.descId);
    input.addEventListener('change', () => {
      set(input.checked);
      stateText.textContent = ON_OFF(input.checked);
    });
    row.refresh = () => {
      input.checked = !!get();
      stateText.textContent = ON_OFF(input.checked);
    };
    return row;
  }

  function buildSelect(item) {
    const get = item.get || (() => cfg()[item.key]);
    const set = item.set || ((v) => { cfg()[item.key] = v; });
    const select = h('select');
    const wrap = h('div', { class: 'select' }, select);
    const row = rowShell(item, wrap);
    select.setAttribute('aria-labelledby', row.labelId);
    if (row.descId) select.setAttribute('aria-describedby', row.descId);
    select.addEventListener('change', () => {
      set(Number(select.value));
      scheduleSave();
      refreshRows();
    });
    row.refresh = () => {
      const options = item.options.filter(item.filter || (() => true));
      select.replaceChildren(...options.map(([v, label]) => h('option', { value: v, text: label })));
      select.value = String(get());
    };
    return row;
  }

  function buildSegmented(item) {
    const group = h('div', { class: 'segmented', role: 'radiogroup' });
    const thumb = h('span', { class: 'thumb', 'aria-hidden': 'true' });
    group.append(thumb);
    const options = item.options.filter(item.filter || (() => true));
    const buttons = options.map(([v, label]) => {
      const off = item.optionDisabled ? item.optionDisabled(v) : false;
      const button = h('button', { type: 'button', role: 'radio', 'data-value': v, text: label,
        'aria-disabled': off ? 'true' : null, class: off ? 'unavailable' : null });
      button.addEventListener('click', () => {
        if (!(item.optionDisabled && item.optionDisabled(v))) choose(v, true);
      });
      group.append(button);
      return button;
    });
    const note = h('div', { class: 'row-desc' });
    const row = rowShell(item, group, note);
    group.setAttribute('aria-labelledby', row.labelId);
    const place = () => {
      const current = buttons.find((b) => b.getAttribute('aria-checked') === 'true');
      if (!current) return;
      thumb.style.setProperty('left', `${current.offsetLeft}px`);
      thumb.style.setProperty('width', `${current.offsetWidth}px`);
    };
    function choose(v, user) {
      if (user) {
        cfg()[item.key] = v;
        scheduleSave();
      }
      for (const b of buttons) {
        const on = Number(b.dataset.value) === v;
        b.setAttribute('aria-checked', on ? 'true' : 'false');
        b.tabIndex = on ? 0 : -1;
        if (on && user) b.focus();
      }
      const recommended = item.recommended === v ? ' ' : '';
      note.textContent = item.note ? item.note(v) + recommended : '';
      requestAnimationFrame(place);
    }
    group.addEventListener('keydown', (e) => {
      const keys = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 };
      if (!(e.key in keys)) return;
      e.preventDefault();
      const values = options.map(([v]) => v).filter((v) => !(item.optionDisabled && item.optionDisabled(v)));
      const index = values.indexOf(cfg()[item.key]);
      choose(values[(index + keys[e.key] + values.length) % values.length], true);
    });
    new ResizeObserver(place).observe(group);
    // 今の値が選べない段階(追加の AI のない High・Unreal など)なら、実際に動いている Standard を選んで見せる。
    row.refresh = () => {
      const v = cfg()[item.key];
      choose(options.some(([o]) => o === v) ? v : (item.fallback ?? v), false);
    };
    return row;
  }

  function buildStepper(item) {
    const out = h('output', { 'aria-live': 'polite' });
    const minus = h('button', { type: 'button', 'aria-label': `${item.label}を減らす`, text: '−' });
    const plus = h('button', { type: 'button', 'aria-label': `${item.label}を増やす`, text: '+' });
    const row = rowShell(item, h('div', { class: 'stepper' }, minus, out, plus));
    const step = (d) => {
      const v = Math.min(item.max, Math.max(item.min, cfg()[item.key] + d));
      if (v === cfg()[item.key]) return;
      cfg()[item.key] = v;
      scheduleSave();
      row.refresh();
    };
    minus.addEventListener('click', () => step(-1));
    plus.addEventListener('click', () => step(1));
    row.refresh = () => {
      const v = cfg()[item.key];
      out.textContent = `${v} ${item.unit}`;
      minus.disabled = v <= item.min;
      plus.disabled = v >= item.max;
    };
    return row;
  }

  function buildChecks(item) {
    const chips = item.bits.map(([bit, label, name]) => {
      const chip = h('button', { type: 'button', class: 'chip', 'aria-pressed': 'false', 'aria-label': `${name}で変換する` },
        h('span', { class: 'chip-mark', 'aria-hidden': 'true' }, icon('check')), label);
      chip.addEventListener('click', () => {
        const on = chip.getAttribute('aria-pressed') !== 'true';
        cfg()[item.key] = on ? (cfg()[item.key] | bit) : (cfg()[item.key] & ~bit);
        scheduleSave();
        row.refresh();
      });
      return [bit, chip];
    });
    const row = rowShell(item, h('div', { class: 'chips', role: 'group' }, chips.map(([, chip]) => chip)));
    row.refresh = () => {
      for (const [bit, chip] of chips) chip.setAttribute('aria-pressed', (cfg()[item.key] & bit) !== 0 ? 'true' : 'false');
    };
    return row;
  }

  function buildText(item) {
    const input = h('input', { class: 'text-input', type: 'text', placeholder: item.placeholder, spellcheck: 'false' });
    const browse = item.browse ? h('button', { class: 'btn', type: 'button', text: '参照…' }) : null;
    const row = rowShell(item, [input, browse]);
    input.setAttribute('aria-labelledby', row.labelId);
    input.addEventListener('change', () => {
      cfg()[item.key] = input.value.trim();
      scheduleSave();
    });
    if (browse) {
      browse.addEventListener('click', async () => {
        const result = await call('open', { target: 'browseModel' });
        if (result.ok && result.path) {
          input.value = result.path;
          cfg()[item.key] = result.path;
          scheduleSave();
        }
      });
    }
    row.refresh = () => {
      if (document.activeElement !== input) input.value = cfg()[item.key] || '';
    };
    return row;
  }

  function buildButton(item) {
    const button = h('button', { class: `btn${item.danger ? ' danger' : ''}`, type: 'button' },
      item.shield ? icon('shield', 'shield') : null, item.button);
    button.addEventListener('click', item.action);
    const row = rowShell(item, button);
    button.setAttribute('aria-describedby', row.labelId);
    row.refresh = () => {
      const desc = row.querySelector('.row-desc');
      if (desc && typeof item.desc === 'function') desc.textContent = item.desc();
    };
    return row;
  }

  // 直近の変換(誤変換の報告)。設定がオンのときだけ、変換サーバーが残した記録を読む(カード 92)。
  function buildRecent(item) {
    const status = h('div', { class: 'row-desc' });
    const list = h('div', { class: 'recent-list' });
    const reload = h('button', { class: 'btn', type: 'button', text: '今の記録を見る' });
    const exportButton = h('button', { class: 'btn', type: 'button', text: '書き出す…' });
    const clearButton = h('button', { class: 'btn danger', type: 'button', text: '消す…' });
    const row = rowShell(item,
      h('div', { class: 'recent-actions' }, reload, exportButton, clearButton),
      h('div', { class: 'recent' }, status, list));
    exportButton.setAttribute('aria-describedby', row.labelId);
    clearButton.setAttribute('aria-describedby', row.labelId);
    let loading = false;
    let lastOn = !!cfg().asteria_recent_conversions;

    async function load() {
      if (!cfg().asteria_recent_conversions) {
        status.textContent = '上のスイッチを入れると、ここに直近の変換が出ます。まだ何も保存していません。';
        list.replaceChildren();
        exportButton.disabled = clearButton.disabled = true;
        return;
      }
      if (loading) return;
      loading = true;
      const result = await call('recentConversions');
      loading = false;
      if (!result.ok) {
        status.textContent = result.message || '記録を読めませんでした。';
        return;
      }
      const entries = result.entries || [];
      exportButton.disabled = clearButton.disabled = entries.length === 0;
      status.textContent = entries.length
        ? `${entries.length} 件あります。新しい順に 5 件を出します。`
        : 'まだ記録がありません。変換して確定すると増えます。';
      list.replaceChildren(...entries.slice(-5).reverse().map((entry) => h('div', { class: 'recent-item' },
        h('div', { class: 'recent-head' },
          h('span', { class: 'recent-reading', text: entry.reading }),
          h('span', { class: 'recent-arrow', text: '→' }),
          h('span', { class: 'recent-committed', text: entry.committed })),
        h('div', { class: 'recent-meta', text: [
          entry.firstKept ? '1 番目で確定' : '候補を選び直した',
          entry.preceding ? `前: ${entry.preceding}` : '',
        ].filter(Boolean).join(' ・ ') }),
        (entry.candidates || []).length
          ? h('div', { class: 'recent-candidates', text: (entry.candidates || []).join(' / ') })
          : null)));
    }

    reload.addEventListener('click', load);
    exportButton.addEventListener('click', async () => {
      const result = await call('exportRecentConversions');
      if (!result.ok) return toast(result.message, 'bad');
      toast(result.saved ? '書き出しました。' : '書き出しをやめました。');
    });
    clearButton.addEventListener('click', async () => {
      if (!await confirmDialog('直近の変換の記録を消しますか?', 'この PC に残した記録を消します。元には戻せません。', '消す', true)) return;
      const result = await call('clear', { what: 'recentConversions' });
      if (!result.ok) return toast(result.message, 'bad');
      toast('消しました。');
      load();
    });
    row.refresh = () => {
      const on = !!cfg().asteria_recent_conversions;
      if (!on) exportButton.disabled = clearButton.disabled = true;
      if (on && !lastOn) load();
      lastOn = on;
    };
    load();
    return row;
  }

  function buildLive(item) {
    const status = h('div', { class: 'row-desc', id: item.id });
    const row = rowShell(item, null, status);
    row.refresh = () => {
      const desc = row.querySelector('.row-desc:not([id="' + item.id + '"])');
      if (desc) desc.textContent = gpuChips().join(' ・ ');
      const st = state.status;
      // 動いていないときだけ理由の文を出し、ふだんは短い言葉にする。
      status.textContent = !st ? '確かめています' : st.state === 'error' ? st.ai : (STATE_WORDS[st.state] || ['確かめています'])[0];
    };
    return row;
  }

  const FORM_GROUPS = {
    'ア': 'カタカナ', 'A': '英字', '0': '数字', '(){}[]': 'かっこ', '.,': 'ピリオド・カンマ',
    '。、': '句読点', '・「」': '中黒・かぎかっこ', '"\'': '引用符', ':;': 'コロン・セミコロン',
    '#%&@$^_|`\\': '記号', '~': 'チルダ', '<>=+-/*': '計算の記号', '?!': '疑問符・感嘆符',
  };
  // 名前のない組は「記号」と、その文字を並べて出す。
  const formGroupName = (group) => FORM_GROUPS[group] || `記号 ${[...group].join(' ')}`;
  const formGroupChars = (group) => (FORM_GROUPS[group] && !['ア', '0', 'A'].includes(group) ? [...group].join(' ') : '');
  const FORMS = [[1, '全角'], [0, '半角'], [2, '前回と同じ']];

  function buildForms(item) {
    const table = h('table', { class: 'forms' });
    const wrap = h('div', {},
      h('div', { class: 'row-desc', text: '「前回と同じ」は最後に選んだ形を使います。' }),
      table);
    const row = h('div', { class: 'row', 'data-search': searchText(item) }, h('div', { class: 'row-text' },
      h('div', { class: 'row-label' }, highlight(item.label)), wrap));
    row.refresh = () => {
      const rules = cfg().character_form_rules || [];
      table.replaceChildren(h('tr', {},
        h('th', { text: '文字' }), h('th', { text: '打っている間' }), h('th', { text: '変換した後' })));
      rules.forEach((rule, index) => {
        const name = formGroupName(rule.group);
        const cell = (field) => {
          const group = h('div', { class: 'segmented', role: 'radiogroup', 'aria-label': `${name}の${field === 'preedit' ? '打っている間' : '変換した後'}` });
          const thumb = h('span', { class: 'thumb', 'aria-hidden': 'true' });
          group.append(thumb);
          const fixed = field === 'preedit' && rule.group === 'ア';
          for (const [v, label] of FORMS) {
            const on = rule[field] === v;
            const b = h('button', { type: 'button', role: 'radio', 'aria-checked': on ? 'true' : 'false', text: label, disabled: fixed && !on });
            b.addEventListener('click', () => {
              if (fixed) return;
              cfg().character_form_rules[index][field] = v;
              scheduleSave();
              row.refresh();
            });
            group.append(b);
          }
          requestAnimationFrame(() => {
            const current = group.querySelector('[aria-checked="true"]');
            if (!current) return;
            thumb.style.setProperty('left', `${current.offsetLeft}px`);
            thumb.style.setProperty('width', `${current.offsetWidth}px`);
          });
          return h('td', {}, group);
        };
        const chars = formGroupChars(rule.group);
        table.append(h('tr', {}, h('td', {}, name, chars ? h('span', { class: 'chars', text: chars }) : null), cell('preedit'), cell('conversion')));
      });
    };
    return row;
  }

  const gb = (bytes) => `${(bytes / 2 ** 30).toFixed(1)} GB`;

  function buildModels(item) {
    const list = h('div', { class: 'models' });
    const row = h('div', { class: 'row', 'data-search': searchText(item) }, h('div', { class: 'row-text' },
      h('div', { class: 'row-desc', text: 'High・Unreal で使う大きい AI です。押したときだけインターネットから受け取ります(打った文字は送りません)。' }),
      list));
    row.refresh = () => {
      list.replaceChildren(...(state.models || []).map((m) => {
        const tier = m.tier === 'unreal' ? 'Unreal' : 'High';
        const info = h('div', { class: 'model-info' },
          h('div', { class: 'row-label' }, m.title, h('span', { class: 'tag', text: tier })),
          h('div', { class: 'row-desc', text: `${gb(m.size)} ・ GPU のメモリ約 ${m.vramGb} GB ・ ${m.license}` }));
        const actions = h('div', { class: 'control' });
        const dl = state.downloads[m.id];
        if (m.installed) {
          actions.append(h('span', { class: 'model-ok' }, icon('check'), '入っています'),
            h('button', { class: 'btn danger', type: 'button', text: '削除…', onclick: () => deleteModel(m) }));
        } else if (dl && dl.running) {
          const pct = dl.total ? Math.floor((dl.received / dl.total) * 100) : 0;
          const bar = h('div', { class: 'progress', role: 'progressbar', 'aria-valuemin': '0', 'aria-valuemax': '100', 'aria-valuenow': String(pct), 'aria-label': `${m.title}を受け取っています` },
            h('span', {}));
          bar.firstChild.style.setProperty('width', `${pct}%`);
          actions.append(h('div', { class: 'progress-wrap' }, bar,
            h('span', { class: 'row-desc', text: `${gb(dl.received)} / ${gb(dl.total)}(${pct}%)` })),
          h('button', { class: 'btn', type: 'button', text: '取り消す', onclick: () => call('cancelDownload', { model: m.id }) }));
        } else if (!m.fits) {
          actions.append(h('span', { class: 'row-desc', text: `この PC の GPU(${state.gpuVramGb.toFixed(0)} GB)では動かせません` }));
        } else {
          const resume = m.received ? '続きを受け取る' : `ダウンロード(${gb(m.size)})`;
          actions.append(h('button', { class: 'btn primary', type: 'button', text: resume, onclick: () => downloadModel(m) }));
        }
        return h('div', { class: 'model' }, info, actions);
      }));
    };
    return row;
  }

  async function loadModels() {
    const result = await call('models');
    if (!result.ok) return;
    state.models = result.models;
    state.gpuVramGb = result.gpuVramGb || 0;
    for (const m of state.models) {
      if (m.downloading) state.downloads[m.id] = { running: true, received: m.received || 0, total: m.size };
    }
  }

  async function downloadModel(m) {
    if (!await confirmDialog(`${m.title} をダウンロードしますか?`,
      `大きさ ${gb(m.size)}。Hugging Face から受け取り、決めた指紋と一致したものだけを使います。\n` +
      `受け取るだけで、打った文字・覚えた言葉・使い方は送りません。\nライセンス: ${m.license}(${m.source})`,
      'ダウンロード', false)) return;
    state.downloads[m.id] = { running: true, received: m.received || 0, total: m.size };
    refreshRows();
    const result = await call('download', { model: m.id });
    if (!result.ok) {
      state.downloads[m.id] = null;
      toast(result.message, 'bad');
      refreshRows();
    }
  }

  async function deleteModel(m) {
    if (!await confirmDialog(`${m.title} を消しますか?`, `${gb(m.size)} を空けます。必要になったら、もう一度ダウンロードできます。`, '消す', true)) return;
    const result = await call('deleteModel', { model: m.id });
    if (!result.ok) {
      toast(result.message, 'bad');
      return;
    }
    if (result.config) state.config = result.config;
    await loadModels();
    render();
    toast('消しました。');
  }

  async function onPushed(e) {
    if (e.event === 'download') {
      state.downloads[e.id] = { running: true, received: e.received, total: e.total };
      refreshRows();
    } else if (e.event === 'downloadDone') {
      state.downloads[e.id] = null;
      await loadModels();
      render();
      if (e.state === 'done') toast('受け取りました。変換の品質で選べます。');
      else if (e.state === 'error') toast(e.message || '受け取れませんでした。', 'bad');
    }
  }

  function buildRow(item) {
    let row;
    switch (item.type) {
      case 'switch':
        row = buildSwitch(item, () => cfg()[item.key], (v) => {
          cfg()[item.key] = v;
          scheduleSave();
          refreshRows();
        });
        break;
      case 'env':
        row = buildSwitch(item, () => state.env[item.key], (v) => {
          state.env[item.key] = v;
          scheduleSave({ [item.key]: v });
        });
        break;
      case 'select': row = buildSelect(item); break;
      case 'segmented': row = buildSegmented(item); break;
      case 'stepper': row = buildStepper(item); break;
      case 'checks': row = buildChecks(item); break;
      case 'text': row = buildText(item); break;
      case 'button': row = buildButton(item); break;
      case 'live': row = buildLive(item); break;
      case 'forms': row = buildForms(item); break;
      case 'models': row = buildModels(item); break;
      case 'recent': row = buildRecent(item); break;
      default: return null;
    }
    row.item = item;
    rows.push(row);
    return row;
  }

  function refreshRows() {
    for (const row of rows) {
      if (row.refresh) row.refresh();
      const disabled = row.item.disabledWhen ? row.item.disabledWhen() : false;
      row.classList.toggle('disabled', disabled);
      row.setAttribute('aria-disabled', disabled ? 'true' : 'false');
      for (const el of row.querySelectorAll('.control input, .control select, .control button')) {
        if (disabled) el.setAttribute('tabindex', '-1');
        else if (!el.closest('.segmented')) el.removeAttribute('tabindex');
      }
    }
  }

  // ---------------------------------------------------------------- 検索
  const normalize = (s) => (s || '').normalize('NFKC').toLowerCase();

  function searchText(item) {
    return normalize([item.label, text(item.desc), item.example, item.search,
      ...(item.options || []).map(([, label]) => label)].join(' '));
  }

  function highlight(value) {
    const q = state.query.trim();
    if (!q || !value) return value || '';
    const source = String(value);
    const index = normalize(source).indexOf(normalize(q));
    if (index < 0 || normalize(source).length !== source.length) return source;
    const frag = document.createDocumentFragment();
    frag.append(source.slice(0, index), h('mark', { text: source.slice(index, index + q.length) }), source.slice(index + q.length));
    return frag;
  }

  function matches(page, group, item) {
    const q = normalize(state.query.trim());
    if (!q) return true;
    return searchText(item).includes(q) || normalize(group.title).includes(q) || normalize(page.title).includes(q);
  }

  // ---------------------------------------------------------------- 画面
  function renderNav() {
    const nav = $('#nav');
    nav.replaceChildren();
    const q = state.query.trim();
    for (const page of PAGES) {
      let count = 0;
      if (q && page.groups) {
        for (const group of page.groups) for (const item of group.items) if (matches(page, group, item)) count++;
      }
      if (q && !count) continue;
      const button = h('button', {
        type: 'button', 'aria-current': !q && state.page === page.id ? 'page' : null,
        onclick: () => go(page.id),
      }, icon(page.icon), h('span', { text: page.title }), q ? h('span', { class: 'count', text: String(count) }) : null);
      nav.append(h('li', {}, button));
    }
  }

  function renderGroups(page, groups, container) {
    for (const group of groups) {
      if (group.when && !group.when()) continue;
      const items = group.items.filter((item) => matches(page, group, item));
      if (!items.length) continue;
      const card = h('div', { class: 'card' }, items.map(buildRow));
      container.append(h('section', { class: 'group', 'aria-label': group.title },
        h('h2', { class: 'group-title', text: group.title }), card));
    }
  }

  function render() {
    rows.length = 0;
    const main = $('#main');
    const q = state.query.trim();
    let page;
    if (q) {
      page = h('div', { class: 'page' }, h('div', { class: 'page-head' },
        h('h1', { class: 'page-title', text: `「${q}」の検索結果` })));
      let any = false;
      for (const p of PAGES) {
        if (!p.groups) continue;
        const before = page.childElementCount;
        renderGroups(p, p.groups.map((g) => ({ ...g, title: `${p.title} › ${g.title}` })), page);
        any = any || page.childElementCount > before;
      }
      if (!any) {
        page.append(h('div', { class: 'results-empty' },
          h('p', { text: '見つかりませんでした。' }),
          h('p', { class: 'note', text: '別の言葉(例: 予測、ライブ、全角、学習)で探してみてください。' })));
      }
    } else {
      const def = PAGES.find((p) => p.id === state.page);
      page = h('div', { class: 'page' });
      if (def.render) {
        def.render(page);
      } else {
        page.append(h('div', { class: 'page-head' },
          h('h1', { class: 'page-title', text: def.title }),
          def.lead ? h('p', { class: 'page-lead', text: def.lead }) : null));
        renderGroups(def, def.groups, page);
      }
    }
    main.replaceChildren(page);
    renderNav();
    refreshRows();
  }

  function go(id, focusKey) {
    if (state.query) {
      state.query = '';
      $('#search').value = '';
    }
    const changed = state.page !== id;
    state.page = id;
    render();
    if (changed) $('#main').scrollTop = 0;
    if (focusKey) {
      const row = rows.find((r) => r.item.key === focusKey);
      if (row) {
        row.scrollIntoView({ block: 'center' });
        row.classList.add('flash');
      }
    }
    if (id === 'home' || id === 'ai') refreshStatus();
  }

  // ---------------------------------------------------------------- ホーム
  const homeEls = {};

  function renderHome(page) {
    const hero = h('section', { class: 'hero', 'aria-label': 'AI の状態' });
    hero.append(h('span', {
      class: 'hero-moon', 'aria-hidden': 'true',
      html: '<svg viewBox="0 0 32 32"><path fill="currentColor" d="M27.22 20.25A12 12 0 1 1 13.77 4.21A10.6 10.6 0 1 0 27.22 20.25Z"/></svg>',
    }));
    homeEls.dot = h('span', { class: 'dot', 'aria-hidden': 'true' });
    homeEls.state = h('span', { text: '確かめています' });
    homeEls.meta = h('div', { class: 'hero-meta' });
    homeEls.ai = h('p', { class: 'hero-ai', hidden: true });
    hero.append(h('div', { class: 'hero-state' }, homeEls.dot, homeEls.state), homeEls.meta, homeEls.ai);
    page.append(hero);

    homeEls.tiles = h('div', { class: 'tiles' });
    page.append(h('section', { class: 'group', 'aria-label': '今日' },
      h('h2', { class: 'group-title', text: '今日' }), homeEls.tiles));

    page.append(h('div', { class: 'support-line' },
      h('span', { text: '気に入ったら、応援をお願いします。' }),
      h('button', { class: 'btn', type: 'button', text: '応援する', onclick: () => call('open', { target: 'support' }) })));
    updateHome();
  }

  // 「このPCのGPU: NVIDIA GeForce RTX 3060（VRAM 12 GB）。…」から、名前と VRAM だけを取り出す。
  function gpuChips() {
    const m = /GPU:\s*(.+?)\s*[（(]VRAM\s*([\d.]+)\s*GB/.exec(state.env.gpu || '');
    if (!m) return ['CPU で動作'];
    return [m[1].replace(/^NVIDIA\s+|^AMD\s+|^Intel\(R\)\s+/, ''), `VRAM ${m[2]} GB`];
  }

  // 数字の札の短い名前(元の長い名前は、指したときに出す)。
  const TILE_NAMES = ['変換', '待ち時間', '1 番目で確定', '予測で節約'];

  const STATE_WORDS = {
    llm: ['いつでも使えます', 'ok'],
    zenz: ['いつでも使えます', 'ok'],
    loading: ['AI を準備しています…', 'wait'],
    sleep: ['AI はひと休み中です', 'wait'],
    off: ['AI を使わずに変換しています', ''],
    error: ['AI が動いていません(入力は続けられます)', 'bad'],
  };

  // "1,234回"・"3分12秒" を、数(大きく)と単位(小さく)の並びに分ける。
  function splitNumber(value) {
    return (value || '—').match(/[\d,.]+|[^\d,.]+/g) || ['—'];
  }

  function animateNumber(el, to) {
    const target = Number(String(to).replace(/,/g, ''));
    const from = Number(el.dataset.value || 0);
    el.dataset.value = String(target);
    if (!Number.isFinite(target) || target === from || matchMedia('(prefers-reduced-motion: reduce)').matches) {
      el.textContent = to;
      return;
    }
    const decimals = (String(to).split('.')[1] || '').length;
    const start = performance.now();
    const tick = (now) => {
      const t = Math.min(1, (now - start) / 700);
      const eased = 1 - Math.pow(1 - t, 3);
      const v = from + (target - from) * eased;
      el.textContent = t < 1 ? v.toLocaleString('ja-JP', { maximumFractionDigits: decimals, minimumFractionDigits: decimals }) : to;
      if (t < 1) requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  }

  function updateHome() {
    const s = state.status;
    if (!homeEls.state || !homeEls.state.isConnected) return;
    const [words, kind] = (s && STATE_WORDS[s.state]) || ['確かめています', ''];
    homeEls.state.textContent = words;
    homeEls.dot.className = `dot ${kind}`;
    // 動いていないときだけ、理由の文を出す。
    homeEls.ai.hidden = !(s && s.state === 'error');
    homeEls.ai.textContent = s ? s.ai : '';
    const chips = gpuChips();
    if (s && s.memoryGb !== undefined) chips.push(`メモリ ${s.memoryGb.toFixed(1)} GB`);
    homeEls.meta.replaceChildren(...chips.map((c) => h('span', { text: c })));
    const usage = (s && s.usage) || [];
    if (homeEls.tiles.childElementCount !== usage.length) {
      homeEls.tiles.replaceChildren(...usage.map(() => {
        return h('div', { class: 'tile' }, h('div', { class: 'tile-title' }),
          h('div', { class: 'tile-value' }), h('div', { class: 'tile-total' }));
      }));
    }
    usage.forEach((tile, i) => {
      const el = homeEls.tiles.children[i];
      const title = el.querySelector('.tile-title');
      title.textContent = TILE_NAMES[i] || tile.title;
      el.title = tile.title;
      const parts = splitNumber(tile.value);
      const valueEl = el.querySelector('.tile-value');
      const numbers = parts.filter((p) => /^[\d,.]+$/.test(p));
      if (numbers.length === 1 && /^[\d,.]+$/.test(parts[0]) && valueEl.dataset.shape === 'one') {
        // 数が 1 つだけで形が同じなら、数え上げて見せる。
        animateNumber(valueEl.querySelector('.num'), parts[0]);
        valueEl.querySelector('.unit').textContent = parts.slice(1).join('');
      } else {
        valueEl.dataset.shape = numbers.length === 1 && /^[\d,.]+$/.test(parts[0]) ? 'one' : 'many';
        valueEl.replaceChildren(...parts.map((p) => h('span', { class: /^[\d,.]+$/.test(p) ? 'num' : 'unit', text: p })));
        if (valueEl.dataset.shape === 'one' && parts.length === 1) valueEl.append(h('span', { class: 'unit' }));
      }
      el.querySelector('.tile-total').textContent = tile.total;
      el.setAttribute('aria-label', `${tile.title} ${tile.value}、${tile.total}`);
    });
  }

  let statusTimer = 0;
  async function refreshStatus() {
    clearTimeout(statusTimer);
    const result = await call('status');
    if (result.ok) {
      state.status = result;
      updateHome();
      const live = document.getElementById('ai-status');
      if (live) live.textContent = result.state === 'error' ? result.ai : (STATE_WORDS[result.state] || ['確かめています'])[0];
    }
    if (!state.query && (state.page === 'home' || state.page === 'ai') && !document.hidden) {
      statusTimer = setTimeout(refreshStatus, 2000);
    }
  }

  document.addEventListener('visibilitychange', () => {
    if (!document.hidden) refreshStatus();
  });

  // ---------------------------------------------------------------- キー操作
  document.addEventListener('keydown', (e) => {
    if ((e.ctrlKey && e.key.toLowerCase() === 'f') || (e.key === '/' && !e.target.closest('input, textarea, select'))) {
      e.preventDefault();
      $('#search').focus();
      $('#search').select();
      return;
    }
    if (e.ctrlKey && e.key === 'Tab') {
      e.preventDefault();
      const ids = PAGES.map((p) => p.id);
      const i = ids.indexOf(state.page);
      go(ids[(i + (e.shiftKey ? -1 : 1) + ids.length) % ids.length]);
      $('#nav [aria-current="page"]')?.focus();
    }
  });

  const search = $('#search');
  let searchTimer = 0;
  search.addEventListener('input', () => {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(() => {
      state.query = search.value;
      render();
      $('#main').scrollTop = 0;
      if (!state.query) refreshStatus();
    }, 90);
  });
  search.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && search.value) {
      e.stopPropagation();
      search.value = '';
      state.query = '';
      render();
      refreshStatus();
    }
  });

  // ---------------------------------------------------------------- 起動
  async function start() {
    const result = await call('load');
    if (!result.ok) {
      $('#loading').replaceChildren(
        h('div', { text: result.message || '設定を読み込めませんでした。' }),
        h('button', { class: 'btn', type: 'button', text: 'もう一度読み込む', onclick: () => location.reload() }));
      return;
    }
    state.config = result.config;
    state.env = result.env || {};
    lastSavedJson = JSON.stringify(state.config);
    await loadModels();
    $('#foot').textContent = `ビルド ${state.env.version || ''}`;
    $('#loading').hidden = true;
    $('#app').hidden = false;
    const first = location.hash.slice(1);
    go(PAGES.some((p) => p.id === first) ? first : 'home');
  }

  // ---------------------------------------------------------------- 見本(WebView2 の外での確認用)
  const Preview = {
    config: {
      preedit_method: 0, use_keyboard_to_change_preedit_method: false, punctuation_method: 0, symbol_method: 0,
      space_character_form: 0, selection_shortcut: 1, numpad_character_form: 2, session_keymap: -1,
      history_learning_level: 0, use_single_kanji_conversion: true, use_symbol_conversion: true,
      use_emoticon_conversion: true, use_date_conversion: true, use_emoji_conversion: true,
      use_number_conversion: true, use_calculator: true, use_t13n_conversion: true, use_zip_code_conversion: true,
      use_spelling_correction: true, auto_switch_composition_mode: true, use_auto_conversion: false,
      auto_conversion_key: 13, shift_key_mode_switch: 1, use_mode_indicator: true, asteria_share_input_mode: true,
      use_history_suggest: true, use_dictionary_suggest: true, suggestions_size: 3, asteria_suggest_min_chars: 3,
      incognito_mode: false, presentation_mode: false, check_default: true, asteria_lm_enabled: true,
      asteria_lm_quality: 1, asteria_lm_model: 0, asteria_lm_correction: true, asteria_live_conversion: false,
      asteria_homophone_hints: false, asteria_romaji_english: false, asteria_use_integrated_gpu: false,
      asteria_lm_model_path: '', use_local_usage_dictionary: true, asteria_recent_conversions: false,
      character_form_rules: [
        ['ア', 1, 1], ['A', 1, 2], ['0', 1, 2], ['(){}[]', 1, 2], ['.,', 1, 2], ['。、', 1, 1], ['・「」', 1, 1],
        ['"\'', 1, 2], [':;', 1, 2], ['#%&@$^_|`\\', 1, 2], ['~', 1, 2], ['<>=+-/*', 1, 2], ['?!', 1, 2],
      ].map(([group, preedit, conversion]) => ({ group, preedit, conversion })),
    },
    env: {
      version: '3.34.6470.100(見本)', gpu: 'このPCのGPU: NVIDIA GeForce RTX 3060(VRAM 12 GB)。Standard以上を使えます。',
      imeHotKeyDisabled: false, supportReminder: true, keymapCustomized: false, romanTableCustomized: false,
    },
    conversions: 1234,
    recent: [
      {
        time: '2026-11-05T07:00:00+0900', firstKept: false, reading: 'くうかんこうへんちょうき',
        committed: '空間後編長期', preceding: 'うーむ納得いきませんね。',
        candidates: ['空間光変調器', '空間後編長期', '空肝硬変長期'],
      },
      {
        time: '2026-11-05T06:58:00+0900', firstKept: true, reading: 'あしたのかいぎ',
        committed: '明日の会議', preceding: '', candidates: ['明日の会議', '明日の回議'],
      },
    ],
    async call(op, args) {
      await new Promise((r) => setTimeout(r, op === 'save' ? 180 : 40));
      if (op === 'load') return { ok: true, config: structuredClone(this.config), env: structuredClone(this.env) };
      if (op === 'save') {
        Object.assign(this.config, args.config);
        Object.assign(this.env, args.env || {});
        return { ok: true, config: structuredClone(this.config), env: structuredClone(this.env), notices: [] };
      }
      if (op === 'defaults') return { ok: true, config: structuredClone(this.config) };
      if (op === 'status') {
        this.conversions += Math.floor(Math.random() * 3);
        return {
          ok: true, state: 'llm', ai: 'AI の状態: 動いています(変換用の AI と日本語 LLM、GPU: NVIDIA GeForce RTX 3060)。',
          memoryGb: 2.4, usage: [
            { title: '変換した回数', value: `${this.conversions.toLocaleString('ja-JP')}回`, total: '累計 48,210回' },
            { title: '考える時間(Spaceから候補まで)', value: '42 ms', total: '累計の平均 47 ms' },
            { title: '正答率(1番目の候補で確定)', value: '94%', total: '累計 93%' },
            { title: '予測で節約できた時間', value: '3分12秒', total: '累計 2時間41分' },
          ],
        };
      }
      if (op === 'diagnostics') return { ok: true, text: 'Asteria 診断情報(見本)\n版: 3.34.6470.100\n...' };
      if (op === 'recentConversions') return { ok: true, entries: structuredClone(this.recent) };
      if (op === 'exportRecentConversions') return { ok: true, saved: true };
      if (op === 'clear' && args && args.what === 'recentConversions') {
        this.recent = [];
        return { ok: true };
      }
      if (op === 'models') {
        return {
          ok: true, gpuVramGb: 12, models: [
            { id: 'qwen3-4b', tier: 'high', title: 'Qwen3-4B', size: 2889513184, vramGb: 4, license: 'Apache-2.0', source: 'https://huggingface.co/Qwen/Qwen3-4B-GGUF', installed: true, downloading: false, fits: true },
            { id: 'qwen3-8b', tier: 'unreal', title: 'Qwen3-8B', size: 5027783488, vramGb: 8, license: 'Apache-2.0', source: 'https://huggingface.co/Qwen/Qwen3-8B-GGUF', installed: false, downloading: false, fits: true },
          ],
        };
      }
      return { ok: true };
    },
  };

  start();
})();
