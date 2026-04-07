/*
 * ni_runtime.h — Ni Language C Runtime
 *
 * NaN-boxed value representation for generated C code.
 * All Ni values are represented as 64-bit values using NaN boxing:
 *   - Doubles that are NOT NaN are float values
 *   - NaN with specific tag bits encodes other types
 *
 * Tag layout (in the NaN payload bits):
 *   Bits 50-48 = type tag (3 bits)
 *   Bits 47-0  = payload (48 bits, enough for pointers)
 *
 * Type tags:
 *   0 = None
 *   1 = Bool (payload: 0 or 1)
 *   2 = Int (payload: 48-bit signed integer, or pointer to 64-bit)
 *   3 = String (payload: pointer to NiString)
 *   4 = List (payload: pointer to NiList)
 *   5 = Map (payload: pointer to NiMap)
 *   6 = Instance (payload: pointer to NiInstance)
 *   7 = Function (payload: pointer to NiFuncObj)
 */

#ifndef NI_RUNTIME_H
#define NI_RUNTIME_H

#include <stdint.h>
#include <stddef.h>
#include <string.h>
#include <stdio.h>
#include <stdlib.h>
#include <math.h>

/*========================================================================
 * NiValue — NaN-boxed value type
 *========================================================================*/

typedef uint64_t NiValue;

/* NaN box constants */
#define NI_QNAN     ((uint64_t)0x7FFC000000000000ULL)
#define NI_TAG_NONE ((uint64_t)0x0000000000000000ULL)
#define NI_TAG_BOOL ((uint64_t)0x0001000000000000ULL)
#define NI_TAG_INT  ((uint64_t)0x0002000000000000ULL)
#define NI_TAG_STR  ((uint64_t)0x0003000000000000ULL)
#define NI_TAG_LIST ((uint64_t)0x0004000000000000ULL)
#define NI_TAG_MAP  ((uint64_t)0x0005000000000000ULL)
#define NI_TAG_INST ((uint64_t)0x0006000000000000ULL)
#define NI_TAG_FUNC ((uint64_t)0x0007000000000000ULL)

#define NI_PAYLOAD_MASK ((uint64_t)0x0000FFFFFFFFFFFFULL)
#define NI_TAG_MASK     ((uint64_t)0x0007000000000000ULL)

#define NI_NONE (NI_QNAN | NI_TAG_NONE)

/*========================================================================
 * Constructors
 *========================================================================*/

static inline NiValue ni_int(int64_t n) {
    /* For values that fit in 48 bits, encode directly */
    return NI_QNAN | NI_TAG_INT | ((uint64_t)(n & 0xFFFFFFFFFFFF));
}

static inline NiValue ni_float(double d) {
    NiValue v;
    memcpy(&v, &d, sizeof(double));
    return v;
}

static inline NiValue ni_bool(int b) {
    return NI_QNAN | NI_TAG_BOOL | (b ? 1ULL : 0ULL);
}

static inline NiValue ni_string(const char* s) {
    char* copy = strdup(s);
    return NI_QNAN | NI_TAG_STR | ((uint64_t)(uintptr_t)copy & NI_PAYLOAD_MASK);
}

/*========================================================================
 * Type checks
 *========================================================================*/

static inline int ni_is_float(NiValue v) {
    return (v & NI_QNAN) != NI_QNAN;
}

static inline int ni_is_none(NiValue v) {
    return v == NI_NONE;
}

static inline int ni_is_bool(NiValue v) {
    return (v & (NI_QNAN | NI_TAG_MASK)) == (NI_QNAN | NI_TAG_BOOL);
}

static inline int ni_is_int(NiValue v) {
    return (v & (NI_QNAN | NI_TAG_MASK)) == (NI_QNAN | NI_TAG_INT);
}

static inline int ni_is_string(NiValue v) {
    return (v & (NI_QNAN | NI_TAG_MASK)) == (NI_QNAN | NI_TAG_STR);
}

static inline int ni_is_list(NiValue v) {
    return (v & (NI_QNAN | NI_TAG_MASK)) == (NI_QNAN | NI_TAG_LIST);
}

static inline int ni_is_map(NiValue v) {
    return (v & (NI_QNAN | NI_TAG_MASK)) == (NI_QNAN | NI_TAG_MAP);
}

static inline int ni_is_instance(NiValue v) {
    return (v & (NI_QNAN | NI_TAG_MASK)) == (NI_QNAN | NI_TAG_INST);
}

static inline int ni_is_function(NiValue v) {
    return (v & (NI_QNAN | NI_TAG_MASK)) == (NI_QNAN | NI_TAG_FUNC);
}

/*========================================================================
 * Extractors
 *========================================================================*/

static inline double ni_as_float(NiValue v) {
    double d;
    memcpy(&d, &v, sizeof(double));
    return d;
}

static inline int64_t ni_as_int(NiValue v) {
    int64_t raw = (int64_t)(v & NI_PAYLOAD_MASK);
    /* Sign-extend from 48 bits */
    if (raw & 0x800000000000LL) {
        raw |= (int64_t)0xFFFF000000000000LL;
    }
    return raw;
}

static inline int ni_as_bool(NiValue v) {
    return (int)(v & 1);
}

static inline const char* ni_as_string(NiValue v) {
    return (const char*)(uintptr_t)(v & NI_PAYLOAD_MASK);
}

/*========================================================================
 * Truthiness
 *========================================================================*/

static inline int ni_is_truthy(NiValue v) {
    if (ni_is_none(v)) return 0;
    if (ni_is_bool(v)) return ni_as_bool(v);
    if (ni_is_int(v)) return ni_as_int(v) != 0;
    if (ni_is_float(v)) return ni_as_float(v) != 0.0;
    if (ni_is_string(v)) return strlen(ni_as_string(v)) > 0;
    return 1; /* objects are truthy */
}

/*========================================================================
 * Arithmetic operators
 *========================================================================*/

static inline NiValue ni_add(NiValue a, NiValue b) {
    if (ni_is_int(a) && ni_is_int(b))
        return ni_int(ni_as_int(a) + ni_as_int(b));
    if (ni_is_float(a) && ni_is_float(b))
        return ni_float(ni_as_float(a) + ni_as_float(b));
    if (ni_is_int(a) && ni_is_float(b))
        return ni_float((double)ni_as_int(a) + ni_as_float(b));
    if (ni_is_float(a) && ni_is_int(b))
        return ni_float(ni_as_float(a) + (double)ni_as_int(b));
    if (ni_is_string(a) && ni_is_string(b)) {
        const char* sa = ni_as_string(a);
        const char* sb = ni_as_string(b);
        size_t len = strlen(sa) + strlen(sb) + 1;
        char* buf = (char*)malloc(len);
        strcpy(buf, sa);
        strcat(buf, sb);
        NiValue result = NI_QNAN | NI_TAG_STR | ((uint64_t)(uintptr_t)buf & NI_PAYLOAD_MASK);
        return result;
    }
    return NI_NONE; /* type error */
}

static inline NiValue ni_sub(NiValue a, NiValue b) {
    if (ni_is_int(a) && ni_is_int(b))
        return ni_int(ni_as_int(a) - ni_as_int(b));
    if (ni_is_float(a) && ni_is_float(b))
        return ni_float(ni_as_float(a) - ni_as_float(b));
    if (ni_is_int(a) && ni_is_float(b))
        return ni_float((double)ni_as_int(a) - ni_as_float(b));
    if (ni_is_float(a) && ni_is_int(b))
        return ni_float(ni_as_float(a) - (double)ni_as_int(b));
    return NI_NONE;
}

static inline NiValue ni_mul(NiValue a, NiValue b) {
    if (ni_is_int(a) && ni_is_int(b))
        return ni_int(ni_as_int(a) * ni_as_int(b));
    if (ni_is_float(a) && ni_is_float(b))
        return ni_float(ni_as_float(a) * ni_as_float(b));
    if (ni_is_int(a) && ni_is_float(b))
        return ni_float((double)ni_as_int(a) * ni_as_float(b));
    if (ni_is_float(a) && ni_is_int(b))
        return ni_float(ni_as_float(a) * (double)ni_as_int(b));
    return NI_NONE;
}

static inline NiValue ni_div(NiValue a, NiValue b) {
    if (ni_is_int(a) && ni_is_int(b)) {
        int64_t bv = ni_as_int(b);
        if (bv == 0) return NI_NONE; /* division by zero */
        return ni_int(ni_as_int(a) / bv);
    }
    if (ni_is_float(a) && ni_is_float(b))
        return ni_float(ni_as_float(a) / ni_as_float(b));
    if (ni_is_int(a) && ni_is_float(b))
        return ni_float((double)ni_as_int(a) / ni_as_float(b));
    if (ni_is_float(a) && ni_is_int(b))
        return ni_float(ni_as_float(a) / (double)ni_as_int(b));
    return NI_NONE;
}

static inline NiValue ni_mod(NiValue a, NiValue b) {
    if (ni_is_int(a) && ni_is_int(b)) {
        int64_t bv = ni_as_int(b);
        if (bv == 0) return NI_NONE;
        return ni_int(ni_as_int(a) % bv);
    }
    if (ni_is_float(a) && ni_is_float(b))
        return ni_float(fmod(ni_as_float(a), ni_as_float(b)));
    return NI_NONE;
}

static inline NiValue ni_negate(NiValue v) {
    if (ni_is_int(v)) return ni_int(-ni_as_int(v));
    if (ni_is_float(v)) return ni_float(-ni_as_float(v));
    return NI_NONE;
}

static inline NiValue ni_not(NiValue v) {
    return ni_bool(!ni_is_truthy(v));
}

/*========================================================================
 * Comparison operators
 *========================================================================*/

static inline NiValue ni_eq(NiValue a, NiValue b) {
    if (ni_is_int(a) && ni_is_int(b))
        return ni_bool(ni_as_int(a) == ni_as_int(b));
    if (ni_is_float(a) && ni_is_float(b))
        return ni_bool(ni_as_float(a) == ni_as_float(b));
    if (ni_is_int(a) && ni_is_float(b))
        return ni_bool((double)ni_as_int(a) == ni_as_float(b));
    if (ni_is_float(a) && ni_is_int(b))
        return ni_bool(ni_as_float(a) == (double)ni_as_int(b));
    if (ni_is_bool(a) && ni_is_bool(b))
        return ni_bool(ni_as_bool(a) == ni_as_bool(b));
    if (ni_is_none(a) && ni_is_none(b))
        return ni_bool(1);
    if (ni_is_string(a) && ni_is_string(b))
        return ni_bool(strcmp(ni_as_string(a), ni_as_string(b)) == 0);
    return ni_bool(0);
}

static inline NiValue ni_neq(NiValue a, NiValue b) {
    return ni_bool(!ni_as_bool(ni_eq(a, b)));
}

static inline NiValue ni_less_than(NiValue a, NiValue b) {
    if (ni_is_int(a) && ni_is_int(b))
        return ni_bool(ni_as_int(a) < ni_as_int(b));
    if (ni_is_float(a) && ni_is_float(b))
        return ni_bool(ni_as_float(a) < ni_as_float(b));
    if (ni_is_int(a) && ni_is_float(b))
        return ni_bool((double)ni_as_int(a) < ni_as_float(b));
    if (ni_is_float(a) && ni_is_int(b))
        return ni_bool(ni_as_float(a) < (double)ni_as_int(b));
    if (ni_is_string(a) && ni_is_string(b))
        return ni_bool(strcmp(ni_as_string(a), ni_as_string(b)) < 0);
    return ni_bool(0);
}

static inline NiValue ni_greater_than(NiValue a, NiValue b) {
    if (ni_is_int(a) && ni_is_int(b))
        return ni_bool(ni_as_int(a) > ni_as_int(b));
    if (ni_is_float(a) && ni_is_float(b))
        return ni_bool(ni_as_float(a) > ni_as_float(b));
    if (ni_is_int(a) && ni_is_float(b))
        return ni_bool((double)ni_as_int(a) > ni_as_float(b));
    if (ni_is_float(a) && ni_is_int(b))
        return ni_bool(ni_as_float(a) > (double)ni_as_int(b));
    if (ni_is_string(a) && ni_is_string(b))
        return ni_bool(strcmp(ni_as_string(a), ni_as_string(b)) > 0);
    return ni_bool(0);
}

static inline NiValue ni_less_eq(NiValue a, NiValue b) {
    return ni_bool(ni_is_truthy(ni_less_than(a, b)) || ni_is_truthy(ni_eq(a, b)));
}

static inline NiValue ni_greater_eq(NiValue a, NiValue b) {
    return ni_bool(ni_is_truthy(ni_greater_than(a, b)) || ni_is_truthy(ni_eq(a, b)));
}

/*========================================================================
 * Type checking
 *========================================================================*/

static inline NiValue ni_is_type(NiValue v, const char* type_name) {
    if (strcmp(type_name, "Int") == 0) return ni_bool(ni_is_int(v));
    if (strcmp(type_name, "Float") == 0) return ni_bool(ni_is_float(v));
    if (strcmp(type_name, "Bool") == 0) return ni_bool(ni_is_bool(v));
    if (strcmp(type_name, "String") == 0) return ni_bool(ni_is_string(v));
    if (strcmp(type_name, "None") == 0) return ni_bool(ni_is_none(v));
    if (strcmp(type_name, "List") == 0) return ni_bool(ni_is_list(v));
    if (strcmp(type_name, "Map") == 0) return ni_bool(ni_is_map(v));
    return ni_bool(0);
}

/*========================================================================
 * String operations
 *========================================================================*/

static inline NiValue ni_str_concat(NiValue a, NiValue b) {
    /* Both should be strings */
    if (ni_is_string(a) && ni_is_string(b)) {
        const char* sa = ni_as_string(a);
        const char* sb = ni_as_string(b);
        size_t len = strlen(sa) + strlen(sb) + 1;
        char* buf = (char*)malloc(len);
        strcpy(buf, sa);
        strcat(buf, sb);
        return NI_QNAN | NI_TAG_STR | ((uint64_t)(uintptr_t)buf & NI_PAYLOAD_MASK);
    }
    return NI_NONE;
}

/*========================================================================
 * Conversion
 *========================================================================*/

static inline NiValue ni_to_string(NiValue v) {
    char buf[64];
    if (ni_is_int(v)) {
        snprintf(buf, sizeof(buf), "%lld", (long long)ni_as_int(v));
        return ni_string(buf);
    }
    if (ni_is_float(v)) {
        snprintf(buf, sizeof(buf), "%g", ni_as_float(v));
        return ni_string(buf);
    }
    if (ni_is_bool(v)) {
        return ni_string(ni_as_bool(v) ? "true" : "false");
    }
    if (ni_is_none(v)) {
        return ni_string("none");
    }
    if (ni_is_string(v)) return v;
    return ni_string("<object>");
}

static inline NiValue ni_to_int(NiValue v) {
    if (ni_is_int(v)) return v;
    if (ni_is_float(v)) return ni_int((int64_t)ni_as_float(v));
    if (ni_is_bool(v)) return ni_int(ni_as_bool(v) ? 1 : 0);
    return ni_int(0);
}

static inline NiValue ni_to_float(NiValue v) {
    if (ni_is_float(v)) return v;
    if (ni_is_int(v)) return ni_float((double)ni_as_int(v));
    return ni_float(0.0);
}

/*========================================================================
 * VM context (opaque, provided by host)
 *========================================================================*/

typedef struct NiVm NiVm;

/*========================================================================
 * Print
 *========================================================================*/

static inline NiValue ni_print(NiVm* vm, NiValue v) {
    (void)vm;
    NiValue s = ni_to_string(v);
    if (ni_is_string(s)) {
        printf("%s\n", ni_as_string(s));
    }
    return NI_NONE;
}

/*========================================================================
 * Type query
 *========================================================================*/

static inline NiValue ni_type_of(NiValue v) {
    if (ni_is_int(v)) return ni_string("Int");
    if (ni_is_float(v)) return ni_string("Float");
    if (ni_is_bool(v)) return ni_string("Bool");
    if (ni_is_none(v)) return ni_string("None");
    if (ni_is_string(v)) return ni_string("String");
    if (ni_is_list(v)) return ni_string("List");
    if (ni_is_map(v)) return ni_string("Map");
    if (ni_is_instance(v)) return ni_string("Instance");
    if (ni_is_function(v)) return ni_string("Function");
    return ni_string("Unknown");
}

/*========================================================================
 * Collections (stub implementations — to be extended)
 *========================================================================*/

/* List - stored as a simple dynamic array on the heap */
typedef struct {
    NiValue* items;
    int len;
    int cap;
} NiList;

static inline NiValue ni_list(NiValue* items, int count) {
    NiList* list = (NiList*)malloc(sizeof(NiList));
    list->len = count;
    list->cap = count > 8 ? count : 8;
    list->items = (NiValue*)malloc(sizeof(NiValue) * list->cap);
    for (int i = 0; i < count; i++) {
        list->items[i] = items[i];
    }
    return NI_QNAN | NI_TAG_LIST | ((uint64_t)(uintptr_t)list & NI_PAYLOAD_MASK);
}

static inline NiValue ni_len(NiValue v) {
    if (ni_is_list(v)) {
        NiList* list = (NiList*)(uintptr_t)(v & NI_PAYLOAD_MASK);
        return ni_int(list->len);
    }
    if (ni_is_string(v)) {
        return ni_int((int64_t)strlen(ni_as_string(v)));
    }
    return ni_int(0);
}

/* Map - stored as parallel key/value arrays */
typedef struct {
    NiValue* keys;
    NiValue* values;
    int len;
    int cap;
} NiMap;

static inline NiValue ni_map(NiValue* keys, NiValue* values, int count) {
    NiMap* map = (NiMap*)malloc(sizeof(NiMap));
    map->len = count;
    map->cap = count > 8 ? count : 8;
    map->keys = (NiValue*)malloc(sizeof(NiValue) * map->cap);
    map->values = (NiValue*)malloc(sizeof(NiValue) * map->cap);
    for (int i = 0; i < count; i++) {
        map->keys[i] = keys[i];
        map->values[i] = values[i];
    }
    return NI_QNAN | NI_TAG_MAP | ((uint64_t)(uintptr_t)map & NI_PAYLOAD_MASK);
}

/*========================================================================
 * Property access (stub — for instances/maps)
 *========================================================================*/

static inline NiValue ni_get_prop(NiValue obj, const char* name) {
    if (ni_is_map(obj)) {
        NiMap* map = (NiMap*)(uintptr_t)(obj & NI_PAYLOAD_MASK);
        NiValue key = ni_string(name);
        for (int i = 0; i < map->len; i++) {
            if (ni_is_truthy(ni_eq(map->keys[i], key))) {
                return map->values[i];
            }
        }
    }
    return NI_NONE;
}

static inline NiValue ni_set_prop(NiValue obj, const char* name, NiValue val) {
    if (ni_is_map(obj)) {
        NiMap* map = (NiMap*)(uintptr_t)(obj & NI_PAYLOAD_MASK);
        NiValue key = ni_string(name);
        for (int i = 0; i < map->len; i++) {
            if (ni_is_truthy(ni_eq(map->keys[i], key))) {
                map->values[i] = val;
                return val;
            }
        }
        /* Add new entry */
        if (map->len >= map->cap) {
            map->cap *= 2;
            map->keys = (NiValue*)realloc(map->keys, sizeof(NiValue) * map->cap);
            map->values = (NiValue*)realloc(map->values, sizeof(NiValue) * map->cap);
        }
        map->keys[map->len] = key;
        map->values[map->len] = val;
        map->len++;
    }
    return val;
}

/*========================================================================
 * Index access
 *========================================================================*/

static inline NiValue ni_get_index(NiValue collection, NiValue index) {
    if (ni_is_list(collection) && ni_is_int(index)) {
        NiList* list = (NiList*)(uintptr_t)(collection & NI_PAYLOAD_MASK);
        int64_t i = ni_as_int(index);
        if (i < 0) i += list->len;
        if (i >= 0 && i < list->len) return list->items[i];
    }
    return NI_NONE;
}

static inline NiValue ni_set_index(NiValue collection, NiValue index, NiValue value) {
    if (ni_is_list(collection) && ni_is_int(index)) {
        NiList* list = (NiList*)(uintptr_t)(collection & NI_PAYLOAD_MASK);
        int64_t i = ni_as_int(index);
        if (i < 0) i += list->len;
        if (i >= 0 && i < list->len) {
            list->items[i] = value;
        }
    }
    return value;
}

/*========================================================================
 * Range
 *========================================================================*/

typedef struct {
    int64_t start;
    int64_t end;
    int inclusive;
    int64_t step;
} NiRangeObj;

static inline NiValue ni_make_range(NiValue start, NiValue end, int inclusive) {
    /* Store as a list-like structure for simplicity */
    NiRangeObj* r = (NiRangeObj*)malloc(sizeof(NiRangeObj));
    r->start = ni_is_int(start) ? ni_as_int(start) : 0;
    r->end = ni_is_int(end) ? ni_as_int(end) : 0;
    r->inclusive = inclusive;
    r->step = 1;
    /* Encode as a tagged pointer — reuse LIST tag with a flag */
    return NI_QNAN | NI_TAG_LIST | ((uint64_t)(uintptr_t)r & NI_PAYLOAD_MASK);
}

/*========================================================================
 * Iterator
 *========================================================================*/

typedef struct {
    int kind; /* 0=range, 1=list, 2=map, 3=string */
    int64_t current;
    int64_t end;
    int inclusive;
    int64_t step;
    void* data;
} NiIterator;

static inline NiIterator ni_get_iterator(NiValue v) {
    NiIterator iter = {0, 0, 0, 0, 1, NULL};
    if (ni_is_list(v)) {
        NiList* list = (NiList*)(uintptr_t)(v & NI_PAYLOAD_MASK);
        iter.kind = 1;
        iter.current = 0;
        iter.end = list->len;
        iter.step = 1;
        iter.data = list;
    }
    /* Extend for other types as needed */
    return iter;
}

static inline int ni_iterator_next(NiIterator* iter, NiValue* out) {
    switch (iter->kind) {
    case 0: /* range */
        if (iter->step > 0) {
            if (!(iter->inclusive ? iter->current <= iter->end : iter->current < iter->end))
                return 0;
        } else {
            if (!(iter->inclusive ? iter->current >= iter->end : iter->current > iter->end))
                return 0;
        }
        *out = ni_int(iter->current);
        iter->current += iter->step;
        return 1;
    case 1: { /* list */
        NiList* list = (NiList*)iter->data;
        if (iter->current < list->len) {
            *out = list->items[iter->current++];
            return 1;
        }
        return 0;
    }
    default:
        return 0;
    }
}

static inline int ni_iterator_next_pair(NiIterator* iter, NiValue* key, NiValue* val) {
    switch (iter->kind) {
    case 1: { /* list */
        NiList* list = (NiList*)iter->data;
        if (iter->current < list->len) {
            *key = ni_int(iter->current);
            *val = list->items[iter->current++];
            return 1;
        }
        return 0;
    }
    case 2: { /* map */
        NiMap* map = (NiMap*)iter->data;
        if (iter->current < map->len) {
            *key = map->keys[iter->current];
            *val = map->values[iter->current++];
            return 1;
        }
        return 0;
    }
    default:
        return 0;
    }
}

/*========================================================================
 * 'in' operator
 *========================================================================*/

static inline NiValue ni_in(NiValue needle, NiValue haystack) {
    if (ni_is_list(haystack)) {
        NiList* list = (NiList*)(uintptr_t)(haystack & NI_PAYLOAD_MASK);
        for (int i = 0; i < list->len; i++) {
            if (ni_is_truthy(ni_eq(list->items[i], needle))) return ni_bool(1);
        }
        return ni_bool(0);
    }
    if (ni_is_string(haystack) && ni_is_string(needle)) {
        return ni_bool(strstr(ni_as_string(haystack), ni_as_string(needle)) != NULL);
    }
    return ni_bool(0);
}

/*========================================================================
 * Function call (dynamic dispatch)
 *========================================================================*/

typedef NiValue (*NiFuncPtr)(NiVm*, NiValue*, int);

typedef struct {
    const char* name;
    NiFuncPtr func;
} NiFuncObj;

static inline NiValue ni_make_function(const char* name, NiFuncPtr func) {
    NiFuncObj* obj = (NiFuncObj*)malloc(sizeof(NiFuncObj));
    obj->name = name;
    obj->func = func;
    return NI_QNAN | NI_TAG_FUNC | ((uint64_t)(uintptr_t)obj & NI_PAYLOAD_MASK);
}

static inline NiValue ni_call(NiVm* vm, NiValue callee, NiValue* args, int argc) {
    if (ni_is_function(callee)) {
        NiFuncObj* func = (NiFuncObj*)(uintptr_t)(callee & NI_PAYLOAD_MASK);
        return func->func(vm, args, argc);
    }
    return NI_NONE;
}

/*========================================================================
 * Method call (stub — for instances)
 *========================================================================*/

static inline NiValue ni_method_call(NiVm* vm, NiValue obj, const char* name,
                                     NiValue* args, int argc) {
    /* Stub: method dispatch for generated code */
    (void)vm; (void)obj; (void)name; (void)args; (void)argc;
    return NI_NONE;
}

/*========================================================================
 * Instance creation (stub)
 *========================================================================*/

static inline NiValue ni_new_instance(const char* class_name, void* vtable) {
    /* Create instance as a map for now */
    NiMap* map = (NiMap*)malloc(sizeof(NiMap));
    map->len = 0;
    map->cap = 8;
    map->keys = (NiValue*)malloc(sizeof(NiValue) * map->cap);
    map->values = (NiValue*)malloc(sizeof(NiValue) * map->cap);
    /* Store class name */
    NiValue inst = NI_QNAN | NI_TAG_INST | ((uint64_t)(uintptr_t)map & NI_PAYLOAD_MASK);
    (void)class_name;
    (void)vtable;
    return inst;
}

/*========================================================================
 * Error handling (stub)
 *========================================================================*/

typedef struct {
    int has_error;
    NiValue value;
} NiErrorState;

static inline NiErrorState ni_push_error_handler(void) {
    NiErrorState state = {0, NI_NONE};
    return state;
}

static inline void ni_pop_error_handler(void) {
    /* no-op stub */
}

static inline NiValue ni_fail(NiValue msg) {
    /* In a real implementation, this would longjmp to the error handler */
    if (ni_is_string(msg)) {
        fprintf(stderr, "RuntimeError: %s\n", ni_as_string(msg));
    }
    return NI_NONE;
}

static inline NiValue ni_try_expr(NiValue v) {
    /* Stub: in real impl, would catch errors */
    return v;
}

#endif /* NI_RUNTIME_H */
