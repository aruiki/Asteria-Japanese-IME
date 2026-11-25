/*
 * llama.cpp の API を、構造体を値渡ししない単純な関数に包む(docs/adr/0006)。
 * Rust 側で llama.cpp の構造体の配置を写さずに済むようにする。
 *
 * Windows で同梱の llama.dll / ggml*.dll (CUDA / Vulkan / CPU-AVX2) が
 * 存在する場合は動的ロードへ切り替え、無い環境(単体テスト・CI)では
 * 静的リンクの llama.cpp へ自動フォールバックする。
 */
#include <math.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _WIN32
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#endif

#include "llama.h"
#include "ggml-backend.h"

#define ASTERIA_LLAMA_BUILD "b11259"

static void asteria_lm_quiet(enum ggml_log_level level, const char * text, void * user_data) {
    (void) level;
    (void) text;
    (void) user_data;
}

typedef void (*fn_llama_log_set)(ggml_log_callback, void *);
typedef void (*fn_llama_backend_init)(void);
typedef struct llama_model_params (*fn_llama_model_default_params)(void);
typedef struct llama_model * (*fn_llama_model_load_from_file)(const char *, struct llama_model_params);
typedef void (*fn_llama_model_free)(struct llama_model *);
typedef const struct llama_vocab * (*fn_llama_model_get_vocab)(const struct llama_model *);
typedef int32_t (*fn_llama_model_meta_val_str)(const struct llama_model *, const char *, char *, size_t);
typedef int32_t (*fn_llama_vocab_n_tokens)(const struct llama_vocab *);
typedef int32_t (*fn_llama_tokenize)(const struct llama_vocab *, const char *, int32_t, llama_token *, int32_t, bool, bool);
typedef const char * (*fn_llama_vocab_get_text)(const struct llama_vocab *, llama_token);
typedef int32_t (*fn_llama_token_to_piece)(const struct llama_vocab *, llama_token, char *, int32_t, int32_t, bool);
typedef llama_token (*fn_llama_vocab_bos)(const struct llama_vocab *);
typedef llama_token (*fn_llama_vocab_eos)(const struct llama_vocab *);
typedef bool (*fn_llama_vocab_get_add_bos)(const struct llama_vocab *);
typedef bool (*fn_llama_vocab_is_eog)(const struct llama_vocab *, llama_token);
typedef struct llama_context_params (*fn_llama_context_default_params)(void);
typedef struct llama_context * (*fn_llama_init_from_model)(struct llama_model *, struct llama_context_params);
typedef void (*fn_llama_free)(struct llama_context *);
typedef llama_memory_t (*fn_llama_get_memory)(const struct llama_context *);
typedef void (*fn_llama_memory_clear)(llama_memory_t, bool);
typedef void (*fn_llama_memory_seq_cp)(llama_memory_t, llama_seq_id, llama_seq_id, llama_pos, llama_pos);
typedef bool (*fn_llama_memory_seq_rm)(llama_memory_t, llama_seq_id, llama_pos, llama_pos);
typedef struct llama_batch (*fn_llama_batch_init)(int32_t, int32_t, int32_t);
typedef void (*fn_llama_batch_free)(struct llama_batch);
typedef int32_t (*fn_llama_decode)(struct llama_context *, struct llama_batch);
typedef void (*fn_llama_synchronize)(struct llama_context *);
typedef float * (*fn_llama_get_logits_ith)(struct llama_context *, int32_t);

typedef void (*fn_ggml_backend_load_all_from_path)(const char *);
typedef ggml_backend_reg_t (*fn_ggml_backend_load)(const char *);
typedef size_t (*fn_ggml_backend_dev_count)(void);
typedef ggml_backend_dev_t (*fn_ggml_backend_dev_get)(size_t);
typedef enum ggml_backend_dev_type (*fn_ggml_backend_dev_type)(ggml_backend_dev_t);
typedef void (*fn_ggml_backend_dev_memory)(ggml_backend_dev_t, size_t *, size_t *);
typedef const char * (*fn_ggml_backend_dev_description)(ggml_backend_dev_t);
typedef const char * (*fn_ggml_backend_dev_name)(ggml_backend_dev_t);

static struct {
    int active;
    int initialized_static;
    int use_gpu;
    int allow_integrated;
    int has_gpu;
    int is_integrated;
    int is_cuda;
    size_t gpu_vram;
    char gpu_name[256];
    ggml_backend_dev_t gpu_dev;

    fn_llama_log_set p_log_set;
    fn_llama_backend_init p_backend_init;
    fn_llama_model_default_params p_model_default_params;
    fn_llama_model_load_from_file p_model_load_from_file;
    fn_llama_model_free p_model_free;
    fn_llama_model_get_vocab p_model_get_vocab;
    fn_llama_model_meta_val_str p_model_meta_val_str;
    fn_llama_vocab_n_tokens p_vocab_n_tokens;
    fn_llama_tokenize p_tokenize;
    fn_llama_vocab_get_text p_vocab_get_text;
    fn_llama_token_to_piece p_token_to_piece;
    fn_llama_vocab_bos p_vocab_bos;
    fn_llama_vocab_eos p_vocab_eos;
    fn_llama_vocab_get_add_bos p_vocab_get_add_bos;
    fn_llama_vocab_is_eog p_vocab_is_eog;
    fn_llama_context_default_params p_context_default_params;
    fn_llama_init_from_model p_init_from_model;
    fn_llama_free p_free;
    fn_llama_get_memory p_get_memory;
    fn_llama_memory_clear p_memory_clear;
    fn_llama_memory_seq_cp p_memory_seq_cp;
    fn_llama_memory_seq_rm p_memory_seq_rm;
    fn_llama_batch_init p_batch_init;
    fn_llama_batch_free p_batch_free;
    fn_llama_decode p_decode;
    fn_llama_synchronize p_synchronize;
    fn_llama_get_logits_ith p_get_logits_ith;

    fn_ggml_backend_load_all_from_path p_load_all;
    fn_ggml_backend_load p_load_backend;
    fn_ggml_backend_dev_count p_dev_count;
    fn_ggml_backend_dev_get p_dev_get;
    fn_ggml_backend_dev_type p_dev_type;
    fn_ggml_backend_dev_memory p_dev_memory;
    fn_ggml_backend_dev_description p_dev_desc;
    fn_ggml_backend_dev_name p_dev_name;
} g_dyn = {0};

#ifdef _WIN32
static HMODULE load_dir_dll(const wchar_t * wdir, const wchar_t * name) {
    size_t dlen = wcslen(wdir);
    size_t nlen = wcslen(name);
    wchar_t * full = (wchar_t *) malloc((dlen + nlen + 2) * sizeof(wchar_t));
    if (!full) return NULL;
    memcpy(full, wdir, dlen * sizeof(wchar_t));
    full[dlen] = L'\\';
    memcpy(full + dlen + 1, name, (nlen + 1) * sizeof(wchar_t));
    HMODULE mod = LoadLibraryExW(
        full, NULL, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (!mod) {
        mod = LoadLibraryExW(full, NULL, LOAD_WITH_ALTERED_SEARCH_PATH);
    }
    free(full);
    return mod;
}

static int is_cuda_name(const char * name) {
    return name != NULL && strncmp(name, "CUDA", 4) == 0;
}

static void try_load_cuda_addon(const char * dir, const wchar_t * wdir) {
    const char * dis_a = getenv("ASTERIA_LM_CUDA");
    const char * dis_k = getenv("KOTORI_LM_CUDA");
    if ((dis_a && strcmp(dis_a, "0") == 0) || (dis_k && strcmp(dis_k, "0") == 0)) {
        return;
    }
    if (!g_dyn.p_load_backend) {
        return;
    }
    size_t dlen = wcslen(wdir);
    wchar_t * wcdir = (wchar_t *) malloc((dlen + 8) * sizeof(wchar_t));
    wchar_t * wstamp = (wchar_t *) malloc((dlen + 32) * sizeof(wchar_t));
    if (!wcdir || !wstamp) {
        free(wcdir);
        free(wstamp);
        return;
    }
    swprintf(wcdir, dlen + 8, L"%ls\\cuda", wdir);
    swprintf(wstamp, dlen + 32, L"%ls\\cuda\\asteria-cuda.txt", wdir);

    FILE * f = _wfopen(wstamp, L"rb");
    free(wstamp);
    if (!f) {
        free(wcdir);
        return;
    }
    char buf[256] = {0};
    size_t n = fread(buf, 1, sizeof(buf) - 1, f);
    fclose(f);
    buf[n] = '\0';

    const char * want = "llama=" ASTERIA_LLAMA_BUILD;
    size_t want_len = strlen(want);
    const char * hit = strstr(buf, want);
    if (!hit) {
        free(wcdir);
        return;
    }
    char after = hit[want_len];
    if (after != '\0' && after != '\r' && after != '\n') {
        free(wcdir);
        return;
    }

    if (load_dir_dll(wcdir, L"ggml-cuda.dll") != NULL) {
        size_t ulen = strlen(dir) + 32;
        char * ucuda = (char *) malloc(ulen);
        if (ucuda) {
            snprintf(ucuda, ulen, "%s\\cuda\\ggml-cuda.dll", dir);
            for (char * p = ucuda; *p != '\0'; p++) {
                if (*p == '/') *p = '\\';
            }
            (void) g_dyn.p_load_backend(ucuda);
            free(ucuda);
        }
    }
    free(wcdir);
}

static void select_best_gpu(int use_gpu, int allow_integrated) {
    g_dyn.use_gpu = use_gpu;
    g_dyn.allow_integrated = allow_integrated;
    g_dyn.has_gpu = 0;
    g_dyn.is_integrated = 0;
    g_dyn.is_cuda = 0;
    g_dyn.gpu_vram = 0;
    g_dyn.gpu_name[0] = '\0';
    g_dyn.gpu_dev = NULL;

    const char * force = getenv("ASTERIA_LM_DEVICE");
    if (!use_gpu || (force && strcmp(force, "cpu") == 0)) {
        return;
    }
    if (!g_dyn.p_dev_count || !g_dyn.p_dev_get || !g_dyn.p_dev_type || !g_dyn.p_dev_memory) {
        return;
    }

    const size_t min_discrete = ((size_t) 3) << 30;
    const size_t min_igpu = ((size_t) 2) << 30;
    const size_t cuda_pref = ((size_t) 1) << 30;
    size_t count = g_dyn.p_dev_count();

    ggml_backend_dev_t best = NULL;
    size_t best_rank = 0;
    size_t best_mem = 0;
    int best_is_cuda = 0;
    int picked_igpu = 0;

    for (size_t i = 0; i < count; i++) {
        ggml_backend_dev_t dev = g_dyn.p_dev_get(i);
        if (g_dyn.p_dev_type(dev) != GGML_BACKEND_DEVICE_TYPE_GPU) continue;
        size_t free_b = 0, total_b = 0;
        g_dyn.p_dev_memory(dev, &free_b, &total_b);
        if (total_b < min_discrete) continue;
        const char * dev_name = g_dyn.p_dev_name ? g_dyn.p_dev_name(dev) : NULL;
        int cur_cuda = is_cuda_name(dev_name);
        size_t rank = total_b + (cur_cuda ? cuda_pref : 0);
        /* 同じ専用 GPU が CUDA と Vulkan で見えた場合は CUDA を優先 (ADR 0088) */
        if (!best || rank > best_rank) {
            best = dev;
            best_rank = rank;
            best_mem = total_b;
            best_is_cuda = cur_cuda;
        }
    }
    if (!best && allow_integrated) {
        for (size_t i = 0; i < count; i++) {
            ggml_backend_dev_t dev = g_dyn.p_dev_get(i);
            if (g_dyn.p_dev_type(dev) != GGML_BACKEND_DEVICE_TYPE_IGPU) continue;
            size_t free_b = 0, total_b = 0;
            g_dyn.p_dev_memory(dev, &free_b, &total_b);
            if (total_b >= min_igpu && total_b > best_mem) {
                best = dev;
                best_mem = total_b;
                picked_igpu = 1;
            }
        }
    }
    if (best) {
        g_dyn.has_gpu = 1;
        g_dyn.is_integrated = picked_igpu;
        g_dyn.is_cuda = best_is_cuda;
        g_dyn.gpu_vram = best_mem;
        g_dyn.gpu_dev = best;
        if (g_dyn.p_dev_desc) {
            const char * desc = g_dyn.p_dev_desc(best);
            if (desc) {
                snprintf(g_dyn.gpu_name, sizeof(g_dyn.gpu_name), "%s", desc);
            }
        }
    }
}
#endif

int32_t asteria_lm_init_runtime(const char * dir, int use_gpu, int allow_integrated) {
#ifdef _WIN32
    if (g_dyn.active) {
        select_best_gpu(use_gpu, allow_integrated);
        return 1;
    }
    if (dir && dir[0] != '\0') {
        int wlen = MultiByteToWideChar(CP_UTF8, 0, dir, -1, NULL, 0);
        if (wlen > 0) {
            wchar_t * wdir = (wchar_t *) malloc((size_t) wlen * sizeof(wchar_t));
            if (wdir) {
                MultiByteToWideChar(CP_UTF8, 0, dir, -1, wdir, wlen);
                for (int i = 0; wdir[i] != L'\0'; i++) {
                    if (wdir[i] == L'/') wdir[i] = L'\\';
                }
                SetDllDirectoryW(wdir);
                (void) load_dir_dll(wdir, L"libomp.dll");
                HMODULE base = load_dir_dll(wdir, L"ggml-base.dll");
                HMODULE ggml = load_dir_dll(wdir, L"ggml.dll");
                HMODULE llama = load_dir_dll(wdir, L"llama.dll");

                if (base && ggml && llama) {
#define RESOLVE_LLAMA(field, sym, ty) \
    g_dyn.field = (ty) (void *) GetProcAddress(llama, #sym); \
    if (!g_dyn.field) goto dyn_fail
#define RESOLVE_MOD(mod, field, sym, ty) \
    g_dyn.field = (ty) (void *) GetProcAddress((mod), #sym); \
    if (!g_dyn.field) goto dyn_fail

                    RESOLVE_LLAMA(p_log_set, llama_log_set, fn_llama_log_set);
                    RESOLVE_LLAMA(p_backend_init, llama_backend_init, fn_llama_backend_init);
                    RESOLVE_LLAMA(p_model_default_params, llama_model_default_params, fn_llama_model_default_params);
                    RESOLVE_LLAMA(p_model_load_from_file, llama_model_load_from_file, fn_llama_model_load_from_file);
                    RESOLVE_LLAMA(p_model_free, llama_model_free, fn_llama_model_free);
                    RESOLVE_LLAMA(p_model_get_vocab, llama_model_get_vocab, fn_llama_model_get_vocab);
                    RESOLVE_LLAMA(p_model_meta_val_str, llama_model_meta_val_str, fn_llama_model_meta_val_str);
                    RESOLVE_LLAMA(p_vocab_n_tokens, llama_vocab_n_tokens, fn_llama_vocab_n_tokens);
                    RESOLVE_LLAMA(p_tokenize, llama_tokenize, fn_llama_tokenize);
                    RESOLVE_LLAMA(p_vocab_get_text, llama_vocab_get_text, fn_llama_vocab_get_text);
                    RESOLVE_LLAMA(p_token_to_piece, llama_token_to_piece, fn_llama_token_to_piece);
                    RESOLVE_LLAMA(p_vocab_bos, llama_vocab_bos, fn_llama_vocab_bos);
                    RESOLVE_LLAMA(p_vocab_eos, llama_vocab_eos, fn_llama_vocab_eos);
                    RESOLVE_LLAMA(p_vocab_get_add_bos, llama_vocab_get_add_bos, fn_llama_vocab_get_add_bos);
                    RESOLVE_LLAMA(p_vocab_is_eog, llama_vocab_is_eog, fn_llama_vocab_is_eog);
                    RESOLVE_LLAMA(p_context_default_params, llama_context_default_params, fn_llama_context_default_params);
                    RESOLVE_LLAMA(p_init_from_model, llama_init_from_model, fn_llama_init_from_model);
                    RESOLVE_LLAMA(p_free, llama_free, fn_llama_free);
                    RESOLVE_LLAMA(p_get_memory, llama_get_memory, fn_llama_get_memory);
                    RESOLVE_LLAMA(p_memory_clear, llama_memory_clear, fn_llama_memory_clear);
                    RESOLVE_LLAMA(p_memory_seq_cp, llama_memory_seq_cp, fn_llama_memory_seq_cp);
                    RESOLVE_LLAMA(p_memory_seq_rm, llama_memory_seq_rm, fn_llama_memory_seq_rm);
                    RESOLVE_LLAMA(p_batch_init, llama_batch_init, fn_llama_batch_init);
                    RESOLVE_LLAMA(p_batch_free, llama_batch_free, fn_llama_batch_free);
                    RESOLVE_LLAMA(p_decode, llama_decode, fn_llama_decode);
                    RESOLVE_LLAMA(p_synchronize, llama_synchronize, fn_llama_synchronize);
                    RESOLVE_LLAMA(p_get_logits_ith, llama_get_logits_ith, fn_llama_get_logits_ith);

                    RESOLVE_MOD(ggml, p_load_all, ggml_backend_load_all_from_path, fn_ggml_backend_load_all_from_path);
                    g_dyn.p_load_backend = (fn_ggml_backend_load) (void *) GetProcAddress(
                        ggml, "ggml_backend_load");
                    RESOLVE_MOD(ggml, p_dev_count, ggml_backend_dev_count, fn_ggml_backend_dev_count);
                    RESOLVE_MOD(ggml, p_dev_get, ggml_backend_dev_get, fn_ggml_backend_dev_get);
                    RESOLVE_MOD(base, p_dev_type, ggml_backend_dev_type, fn_ggml_backend_dev_type);
                    RESOLVE_MOD(base, p_dev_memory, ggml_backend_dev_memory, fn_ggml_backend_dev_memory);
                    g_dyn.p_dev_desc = (fn_ggml_backend_dev_description) (void *) GetProcAddress(
                        base, "ggml_backend_dev_description");
                    g_dyn.p_dev_name = (fn_ggml_backend_dev_name) (void *) GetProcAddress(
                        base, "ggml_backend_dev_name");
#undef RESOLVE_LLAMA
#undef RESOLVE_MOD

                    g_dyn.p_log_set(asteria_lm_quiet, NULL);
                    g_dyn.p_load_all(dir);
                    try_load_cuda_addon(dir, wdir);
                    g_dyn.p_backend_init();
                    g_dyn.active = 1;
                    select_best_gpu(use_gpu, allow_integrated);
                    free(wdir);
                    return 1;
                }
dyn_fail:
                free(wdir);
                memset(&g_dyn, 0, sizeof(g_dyn));
            }
        }
    }
#else
    (void) dir;
    (void) use_gpu;
    (void) allow_integrated;
#endif
    if (!g_dyn.initialized_static) {
        llama_log_set(asteria_lm_quiet, NULL);
        llama_backend_init();
        g_dyn.initialized_static = 1;
    }
    return 0;
}

void asteria_lm_init(void) {
    (void) asteria_lm_init_runtime(NULL, 0, 0);
}

int32_t asteria_lm_using_dynamic(void) {
    return g_dyn.active;
}

int32_t asteria_lm_has_gpu(void) {
    return g_dyn.active && g_dyn.has_gpu;
}

int32_t asteria_lm_gpu_is_integrated(void) {
    return g_dyn.active && g_dyn.is_integrated;
}

int32_t asteria_lm_gpu_is_cuda(void) {
    return g_dyn.active && g_dyn.has_gpu && g_dyn.is_cuda;
}

size_t asteria_lm_gpu_vram_bytes(void) {
    return g_dyn.active ? g_dyn.gpu_vram : 0;
}

const char * asteria_lm_gpu_name(void) {
    return g_dyn.active ? g_dyn.gpu_name : "";
}

struct llama_model * asteria_lm_load(const char * path, int vocab_only) {
    if (g_dyn.active) {
        struct llama_model_params params = g_dyn.p_model_default_params();
        params.vocab_only = vocab_only != 0;
        static ggml_backend_dev_t s_devs[2];
        if (!params.vocab_only && g_dyn.use_gpu && g_dyn.gpu_dev != NULL) {
            s_devs[0] = g_dyn.gpu_dev;
            s_devs[1] = NULL;
            params.devices = s_devs;
            params.n_gpu_layers = 999;
            params.load_mode = LLAMA_LOAD_MODE_NONE;
        } else {
            params.n_gpu_layers = 0;
        }
        return g_dyn.p_model_load_from_file(path, params);
    }
    struct llama_model_params params = llama_model_default_params();
    params.vocab_only = vocab_only != 0;
    params.n_gpu_layers = 0;
    return llama_model_load_from_file(path, params);
}

void asteria_lm_free(struct llama_model * model) {
    if (g_dyn.active) {
        g_dyn.p_model_free(model);
    } else {
        llama_model_free(model);
    }
}

int32_t asteria_lm_n_vocab(const struct llama_model * model) {
    if (g_dyn.active) {
        return g_dyn.p_vocab_n_tokens(g_dyn.p_model_get_vocab(model));
    }
    return llama_vocab_n_tokens(llama_model_get_vocab(model));
}

int32_t asteria_lm_tokenize(const struct llama_model * model, const char * text, int32_t text_len,
                           int32_t * tokens, int32_t n_tokens_max, int add_special) {
    if (g_dyn.active) {
        return g_dyn.p_tokenize(g_dyn.p_model_get_vocab(model), text, text_len, tokens,
                                n_tokens_max, add_special != 0, false);
    }
    return llama_tokenize(llama_model_get_vocab(model), text, text_len, tokens, n_tokens_max,
                          add_special != 0, false);
}

const char * asteria_lm_token_text(const struct llama_model * model, int32_t token) {
    if (g_dyn.active) {
        const struct llama_vocab * vocab = g_dyn.p_model_get_vocab(model);
        if (token < 0 || token >= g_dyn.p_vocab_n_tokens(vocab)) {
            return NULL;
        }
        return g_dyn.p_vocab_get_text(vocab, token);
    }
    const struct llama_vocab * vocab = llama_model_get_vocab(model);
    if (token < 0 || token >= llama_vocab_n_tokens(vocab)) {
        return NULL;
    }
    return llama_vocab_get_text(vocab, token);
}

int32_t asteria_lm_token_to_piece(const struct llama_model * model, int32_t token,
                                  char * buf, int32_t buf_size) {
    if (g_dyn.active) {
        return g_dyn.p_token_to_piece(g_dyn.p_model_get_vocab(model), token, buf, buf_size, 0, false);
    }
    return llama_token_to_piece(llama_model_get_vocab(model), token, buf, buf_size, 0, false);
}

int32_t asteria_lm_meta(const struct llama_model * model, const char * key, char * buf, size_t buf_size) {
    if (g_dyn.active) {
        return g_dyn.p_model_meta_val_str(model, key, buf, buf_size);
    }
    return llama_model_meta_val_str(model, key, buf, buf_size);
}

struct llama_context * asteria_lm_ctx_new(struct llama_model * model, uint32_t n_ctx, uint32_t n_seq,
                                         int32_t n_threads) {
    uint32_t batch_cap = n_ctx < 256 ? n_ctx : 256;
    if (g_dyn.active) {
        struct llama_context_params params = g_dyn.p_context_default_params();
        params.n_ctx = n_ctx;
        params.n_batch = batch_cap;
        params.n_ubatch = batch_cap;
        params.n_seq_max = n_seq;
        params.n_threads = n_threads;
        params.n_threads_batch = n_threads;
        params.kv_unified = true;
        params.flash_attn_type = LLAMA_FLASH_ATTN_TYPE_ENABLED;
        return g_dyn.p_init_from_model(model, params);
    }
    struct llama_context_params params = llama_context_default_params();
    params.n_ctx = n_ctx;
    params.n_batch = batch_cap;
    params.n_ubatch = batch_cap;
    params.n_seq_max = n_seq;
    params.n_threads = n_threads;
    params.n_threads_batch = n_threads;
    params.kv_unified = true;
    return llama_init_from_model(model, params);
}

void asteria_lm_ctx_free(struct llama_context * ctx) {
    if (g_dyn.active) {
        g_dyn.p_free(ctx);
    } else {
        llama_free(ctx);
    }
}

void asteria_lm_clear(struct llama_context * ctx) {
    if (g_dyn.active) {
        g_dyn.p_memory_clear(g_dyn.p_get_memory(ctx), true);
    } else {
        llama_memory_clear(llama_get_memory(ctx), true);
    }
}

void asteria_lm_seq_cp(struct llama_context * ctx, int32_t src, int32_t dst) {
    if (g_dyn.active) {
        g_dyn.p_memory_seq_cp(g_dyn.p_get_memory(ctx), src, dst, -1, -1);
    } else {
        llama_memory_seq_cp(llama_get_memory(ctx), src, dst, -1, -1);
    }
}

void asteria_lm_seq_cp_range(struct llama_context * ctx, int32_t src, int32_t dst, int32_t p0, int32_t p1) {
    if (g_dyn.active) {
        g_dyn.p_memory_seq_cp(g_dyn.p_get_memory(ctx), src, dst, p0, p1);
    } else {
        llama_memory_seq_cp(llama_get_memory(ctx), src, dst, p0, p1);
    }
}

int32_t asteria_lm_seq_rm(struct llama_context * ctx, int32_t seq_id, int32_t p0, int32_t p1) {
    if (g_dyn.active) {
        return g_dyn.p_memory_seq_rm(g_dyn.p_get_memory(ctx), seq_id, p0, p1) ? 1 : 0;
    }
    return llama_memory_seq_rm(llama_get_memory(ctx), seq_id, p0, p1) ? 1 : 0;
}

int32_t asteria_lm_decode(struct llama_context * ctx, int32_t n, const int32_t * tokens,
                         const int32_t * pos, const int32_t * n_seq, const int32_t * seqs,
                         int32_t n_seq_max, const int8_t * want_logits) {
    struct llama_batch batch = g_dyn.active
        ? g_dyn.p_batch_init(n, 0, n_seq_max)
        : llama_batch_init(n, 0, n_seq_max);
    int32_t at = 0;
    for (int32_t i = 0; i < n; i++) {
        batch.token[i] = tokens[i];
        batch.pos[i] = pos[i];
        batch.n_seq_id[i] = n_seq[i];
        for (int32_t s = 0; s < n_seq[i]; s++) {
            batch.seq_id[i][s] = seqs[at++];
        }
        batch.logits[i] = want_logits[i];
    }
    batch.n_tokens = n;
    int32_t ret = g_dyn.active ? g_dyn.p_decode(ctx, batch) : llama_decode(ctx, batch);
    if (ret == 0) {
        if (g_dyn.active) {
            g_dyn.p_synchronize(ctx);
        } else {
            llama_synchronize(ctx);
        }
    }
    if (g_dyn.active) {
        g_dyn.p_batch_free(batch);
    } else {
        llama_batch_free(batch);
    }
    return ret;
}

const float * asteria_lm_logits(struct llama_context * ctx, int32_t i) {
    if (g_dyn.active) {
        return g_dyn.p_get_logits_ith(ctx, i);
    }
    return llama_get_logits_ith(ctx, i);
}

static float asteria_lse_fast(const float * l, int32_t n_vocab) {
    float max_val = l[0];
    for (int32_t i = 1; i < n_vocab; i++) {
        if (l[i] > max_val) max_val = l[i];
    }
    float cut = max_val - 20.0f;
    double sum = 0.0;
    for (int32_t i = 0; i < n_vocab; i++) {
        if (l[i] > cut) sum += exp((double) (l[i] - max_val));
    }
    return max_val + (float) log(sum);
}

int32_t asteria_lm_token_logprobs(struct llama_context * ctx, int32_t batch_idx, int32_t n_vocab,
                                  const int32_t * tokens, int32_t n_tokens, float * out_logp) {
    const float * l = asteria_lm_logits(ctx, batch_idx);
    if (!l || n_vocab <= 0 || n_tokens < 0) return -1;
    if (n_tokens == 0) return 0;
    float lse = asteria_lse_fast(l, n_vocab);
    for (int32_t i = 0; i < n_tokens; i++) {
        int32_t tok = tokens[i];
        if (tok < 0 || tok >= n_vocab) return -1;
        out_logp[i] = l[tok] - lse;
    }
    return 0;
}

int32_t asteria_lm_topk(struct llama_context * ctx, int32_t batch_idx, int32_t n_vocab,
                        int32_t k, int32_t * out_tokens, float * out_logp) {
    const float * l = asteria_lm_logits(ctx, batch_idx);
    if (!l || n_vocab <= 0 || k <= 0) return -1;
    int32_t cap = k < n_vocab ? k : n_vocab;
    float lse = asteria_lse_fast(l, n_vocab);

    int32_t count = 0;
    float floor_val = -1e30f;
    for (int32_t i = 0; i < n_vocab; i++) {
        float v = l[i];
        if (count == cap && v <= floor_val) continue;
        int32_t pos = count;
        while (pos > 0 && l[out_tokens[pos - 1]] < v) {
            if (pos < cap) {
                out_tokens[pos] = out_tokens[pos - 1];
            }
            pos--;
        }
        if (pos < cap) {
            out_tokens[pos] = i;
            if (count < cap) count++;
            if (count == cap) floor_val = l[out_tokens[cap - 1]];
        }
    }
    for (int32_t i = 0; i < count; i++) {
        out_logp[i] = l[out_tokens[i]] - lse;
    }
    return count;
}

int32_t asteria_lm_bos(const struct llama_model * model) {
    if (g_dyn.active) {
        return g_dyn.p_vocab_bos(g_dyn.p_model_get_vocab(model));
    }
    return llama_vocab_bos(llama_model_get_vocab(model));
}

int32_t asteria_lm_eos(const struct llama_model * model) {
    if (g_dyn.active) {
        return g_dyn.p_vocab_eos(g_dyn.p_model_get_vocab(model));
    }
    return llama_vocab_eos(llama_model_get_vocab(model));
}

int32_t asteria_lm_add_bos(const struct llama_model * model) {
    if (g_dyn.active) {
        return g_dyn.p_vocab_get_add_bos(g_dyn.p_model_get_vocab(model)) ? 1 : 0;
    }
    return llama_vocab_get_add_bos(llama_model_get_vocab(model)) ? 1 : 0;
}

int32_t asteria_lm_is_eog(const struct llama_model * model, int32_t token) {
    if (g_dyn.active) {
        return g_dyn.p_vocab_is_eog(g_dyn.p_model_get_vocab(model), token) ? 1 : 0;
    }
    return llama_vocab_is_eog(llama_model_get_vocab(model), token) ? 1 : 0;
}
