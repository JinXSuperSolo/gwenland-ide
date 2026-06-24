// GwenLand IDE — CodeMirror 6 editor bundle entry point.
//
// This file is bundled once by build.sh into dist/codemirror.bundle.js (a
// committed static file) and exposed on the global `GwenEditorBundle`. It is
// never rebuilt by `cargo tauri dev` / `cargo tauri build`; rebuild manually
// only when the CM6 dependencies below change.
//
// No language grammars (@codemirror/lang-*), no LSP-shaped extensions — those
// belong to a later milestone.

import { EditorState } from '@codemirror/state';
import {
    EditorView,
    keymap,
    lineNumbers,
    highlightActiveLine,
    highlightActiveLineGutter,
    drawSelection,
    rectangularSelection,
    crosshairCursor,
    dropCursor,
} from '@codemirror/view';
import {
    defaultKeymap,
    history,
    historyKeymap,
    indentWithTab,
    undo,
    redo,
} from '@codemirror/commands';
import {
    search,
    searchKeymap,
    highlightSelectionMatches,
    getSearchQuery,
    setSearchQuery,
    SearchQuery,
    findNext,
    findPrevious,
    selectMatches,
    replaceNext,
    replaceAll,
    openSearchPanel,
    closeSearchPanel,
} from '@codemirror/search';
import {
    indentOnInput,
    bracketMatching,
    indentUnit,
} from '@codemirror/language';
// NOTE: foldGutter/foldKeymap are intentionally NOT imported. Meaningful code
// folding requires a language grammar (the fold service comes from the language
// facet), which is Milestone 6. Without it the fold arrows render but fold
// nothing useful, so folding is deferred until languages land.

// Small inline SVG icons for the search widget (VS Code style).
const SVG = {
    chevronRight: '<svg viewBox="0 0 16 16" width="14" height="14"><path fill="currentColor" d="M6 4l4 4-4 4V4z"/></svg>',
    chevronDown: '<svg viewBox="0 0 16 16" width="14" height="14"><path fill="currentColor" d="M4 6l4 4 4-4H4z"/></svg>',
    up: '<svg viewBox="0 0 16 16" width="13" height="13"><path fill="currentColor" d="M8 4l4.5 4.5-1 1L8 6l-3.5 3.5-1-1L8 4z"/></svg>',
    down: '<svg viewBox="0 0 16 16" width="13" height="13"><path fill="currentColor" d="M8 12L3.5 7.5l1-1L8 10l3.5-3.5 1 1L8 12z"/></svg>',
    selectAll: '<svg viewBox="0 0 16 16" width="13" height="13"><path fill="currentColor" d="M2 3h12v2H2V3zm0 4h8v2H2V7zm0 4h12v2H2v-2z"/></svg>',
    close: '<svg viewBox="0 0 16 16" width="13" height="13"><path fill="currentColor" d="M8 9.06l3.7 3.7 1.06-1.06L9.06 8l3.7-3.7-1.06-1.06L8 6.94l-3.7-3.7L3.24 4.3 6.94 8l-3.7 3.7 1.06 1.06L8 9.06z"/></svg>',
};

// Custom VS Code-style search panel: a single compact row (find field, match
// count, prev/next/select-all, the Aa / ab / .* toggles, close) plus a
// collapsible replace row toggled by the chevron on the left.
function createSearchPanel(view) {
    const dom = document.createElement('div');
    dom.className = 'gw-search';

    dom.innerHTML = `
        <button class="gw-s-toggle" title="Toggle Replace" aria-label="Toggle Replace">${SVG.chevronRight}</button>
        <div class="gw-s-rows">
            <div class="gw-s-row">
                <div class="gw-s-field">
                    <input class="gw-s-input" name="search" placeholder="Find" aria-label="Find"/>
                    <div class="gw-s-toggles">
                        <button class="gw-s-tog" data-tog="case" title="Match Case">Aa</button>
                        <button class="gw-s-tog" data-tog="word" title="Match Whole Word"><u>ab</u></button>
                        <button class="gw-s-tog" data-tog="re" title="Use Regular Expression">.*</button>
                    </div>
                </div>
                <span class="gw-s-count">No results</span>
                <button class="gw-s-btn" data-act="prev" title="Previous Match (Shift+Enter)">${SVG.up}</button>
                <button class="gw-s-btn" data-act="next" title="Next Match (Enter)">${SVG.down}</button>
                <button class="gw-s-btn" data-act="all" title="Select All Matches">${SVG.selectAll}</button>
                <button class="gw-s-btn gw-s-close" data-act="close" title="Close (Escape)">${SVG.close}</button>
            </div>
            <div class="gw-s-row gw-s-replace-row">
                <div class="gw-s-field">
                    <input class="gw-s-input" name="replace" placeholder="Replace" aria-label="Replace"/>
                </div>
                <button class="gw-s-btn gw-s-text" data-act="replace" title="Replace">Replace</button>
                <button class="gw-s-btn gw-s-text" data-act="replaceAll" title="Replace All">All</button>
            </div>
        </div>
    `;

    const searchInput = dom.querySelector('input[name="search"]');
    const replaceInput = dom.querySelector('input[name="replace"]');
    const countEl = dom.querySelector('.gw-s-count');
    const toggleBtn = dom.querySelector('.gw-s-toggle');
    const togState = { case: false, word: false, re: false };

    function commitQuery(extra) {
        const q = new SearchQuery({
            search: searchInput.value,
            replace: replaceInput.value,
            caseSensitive: togState.case,
            wholeWord: togState.word,
            regexp: togState.re,
        });
        view.dispatch({ effects: setSearchQuery.of(q) });
        if (extra) extra();
        updateCount();
    }

    // Recompute the "X of Y" / "No results" label from the current query.
    function updateCount() {
        const query = getSearchQuery(view.state);
        if (!query.search) { countEl.textContent = 'No results'; countEl.classList.remove('gw-s-has'); return; }
        let cursor, total = 0;
        try {
            cursor = query.getCursor(view.state.doc);
            while (!cursor.next().done) total++;
        } catch (e) { total = 0; }
        if (total === 0) { countEl.textContent = 'No results'; countEl.classList.remove('gw-s-has'); }
        else { countEl.textContent = total + (total === 1 ? ' result' : ' results'); countEl.classList.add('gw-s-has'); }
    }

    toggleBtn.addEventListener('click', () => {
        const open = dom.classList.toggle('gw-s-expanded');
        toggleBtn.innerHTML = open ? SVG.chevronDown : SVG.chevronRight;
        if (open) replaceInput.focus();
    });

    dom.querySelectorAll('.gw-s-tog').forEach((btn) => {
        btn.addEventListener('click', () => {
            const k = btn.dataset.tog;
            togState[k] = !togState[k];
            btn.classList.toggle('gw-s-on', togState[k]);
            commitQuery();
        });
    });

    dom.querySelectorAll('.gw-s-btn').forEach((btn) => {
        btn.addEventListener('click', () => {
            const act = btn.dataset.act;
            if (act === 'next') findNext(view);
            else if (act === 'prev') findPrevious(view);
            else if (act === 'all') selectMatches(view);
            else if (act === 'replace') replaceNext(view);
            else if (act === 'replaceAll') replaceAll(view);
            else if (act === 'close') closeSearchPanel(view);
            view.focus();
            updateCount();
        });
    });

    searchInput.addEventListener('input', () => commitQuery());
    searchInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') { e.preventDefault(); e.shiftKey ? findPrevious(view) : findNext(view); }
        else if (e.key === 'Escape') { e.preventDefault(); closeSearchPanel(view); view.focus(); }
    });
    replaceInput.addEventListener('input', () => commitQuery());
    replaceInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') { e.preventDefault(); replaceNext(view); }
        else if (e.key === 'Escape') { e.preventDefault(); closeSearchPanel(view); view.focus(); }
    });

    return {
        dom,
        top: true,
        mount() {
            // Seed the field from any existing query, then focus + select.
            const q = getSearchQuery(view.state);
            if (q.search) searchInput.value = q.search;
            updateCount();
            searchInput.focus();
            searchInput.select();
        },
        update(update) {
            if (update.docChanged) updateCount();
        },
    };
}

// Build an EditorState for `doc`. `onDocChange` (optional) is called whenever
// the document content changes, so the host can mark the tab dirty.
function createEditorState(doc, onDocChange) {
    const extensions = [
        lineNumbers(),
        highlightActiveLineGutter(),
        highlightActiveLine(),
        drawSelection(),
        dropCursor(),
        rectangularSelection(),
        crosshairCursor(),
        history(),
        indentOnInput(),
        bracketMatching(),
        highlightSelectionMatches(),
        search({ top: true, createPanel: createSearchPanel }),
        indentUnit.of('    '), // 4-space indent
        keymap.of([
            ...defaultKeymap,
            ...historyKeymap,
            ...searchKeymap,
            indentWithTab,
        ]),
        EditorView.updateListener.of((update) => {
            if (update.docChanged && typeof onDocChange === 'function') {
                onDocChange();
            }
        }),
    ];
    return EditorState.create({ doc: doc ?? '', extensions });
}

// Mount a fresh EditorView for `state` inside `parent`, replacing whatever was
// there before. Returns the view so the host can read/destroy it.
function mountEditorView(state, parent) {
    parent.innerHTML = '';
    return new EditorView({ state, parent });
}

// Theme visuals are driven by CSS custom properties on document.documentElement
// (CM6 inherits the page colors), not by CM6 compartments. This nudges the view
// to repaint after a theme switch via a no-op dispatch.
function applyTheme(view, theme) {
    if (!view) return;
    view.dispatch({});
}

export {
    createEditorState,
    mountEditorView,
    applyTheme,
    EditorView,
    EditorState,
    // Command helpers for the Edit menu / shortcuts (act on an EditorView).
    undo,
    redo,
    openSearchPanel,
};
