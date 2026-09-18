(() => {
  'use strict';
  const $ = (s, root = document) => root.querySelector(s);
  const $$ = (s, root = document) => [...root.querySelectorAll(s)];
  const escape = value => String(value).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
  const upstream = window.ALSA_DOCS;
  const guides = window.ALSA_GUIDES;
  const records = [...guides, ...upstream.records];
  const byId = new Map(records.map(r => [r.id, r]));
  const manual = upstream.records.filter(r => r.id.startsWith('manual/'));
  const api = upstream.records.filter(r => r.id.startsWith('api/'));
  const categories = [...new Set(api.map(r => r.category))];
  const normalize = s => s.toLowerCase().replace(/[_\W]+/g, ' ').trim();
  records.forEach(r => {
    const template = document.createElement('template'); template.innerHTML = r.html;
    r.searchText = normalize(r.title + ' ' + r.category + ' ' + (r.text || template.content.textContent));
    r.searchTitle = normalize(r.title);
  });
  const shortTitles = {
    pcm: 'Digital audio fundamentals', pcm_general_overview: 'The ring buffer', pcm_transfer: 'Transfer methods',
    pcm_open_behaviour: 'Opening a device', pcm_async: 'Asynchronous mode', pcm_handshake: 'Stream states',
    pcm_formats: 'Sample formats', alsa_transfers: 'Read, write & mmap', pcm_errors: 'Error codes',
    pcm_params: 'Hardware & software parameters', pcm_status: 'Status & timing', pcm_action: 'Controlling the stream',
    pcm_sync: 'Stream synchronization', pcm_thread_safety: 'Thread safety', pcm_dev_names: 'Device names', pcm_examples: 'Upstream examples'
  };
  function navLink(r, i) {
    const title = shortTitles[r.id.split('/')[1]] || r.title;
    return `<a href="#/${r.id}" data-route="${r.id}">${i !== undefined ? `<span class="nav-number">${String(i + 1).padStart(2, '0')}</span>` : ''}${escape(title)}</a>`;
  }
  $('#navigation').innerHTML = `<div class="nav-group"><div class="nav-label">LEARN <span>Editorial guides</span></div>${guides.map((r,i) => navLink(r,i)).join('')}</div><details class="nav-group" open><summary class="nav-label">PCM MANUAL <span>Upstream</span></summary>${manual.map(r => navLink(r)).join('')}</details><div class="nav-group"><div class="nav-label">REFERENCE</div><a href="#/reference" data-route="reference"><span class="nav-symbol">ƒ</span> Browse the API <span class="count">${api.length}</span></a><a href="#/guide/how-to/scope">About & provenance ↗</a></div>`;
  $('#snapshot-date').textContent = 'Imported ' + upstream.fetched;

  const main = $('#main');
  let observer;
  let currentRoute = '';
  function parseRoute() {
    const raw = location.hash.slice(2) || 'guide/welcome';
    const [path, query = ''] = raw.split('?');
    const parts = path.split('/');
    return { id: parts[0] === 'reference' ? 'reference' : parts.slice(0,2).join('/'), anchor: decodeURIComponent(parts.slice(2).join('/')), query: new URLSearchParams(query) };
  }
  function render() {
    // The skip link is a normal document anchor, not an application route.
    if (location.hash === '#main') { main.focus(); return; }
    const route = parseRoute();
    const samePage = route.id === currentRoute && route.id !== 'reference';
    if (!samePage) {
      currentRoute = route.id;
      if (route.id === 'reference') renderReference(route.query);
      else {
        const record = byId.get(route.id);
        if (!record) {
          main.innerHTML = `<div class="eyebrow">NOT FOUND</div><h1>This page isn’t in the snapshot.</h1><p>Try searching for a function or <a href="#/guide/welcome">return to the start</a>.</p>`;
          document.title = 'Page not found · ALSA PCM';
        } else renderRecord(record);
      }
      setupPage(route.id);
      window.scrollTo(0,0);
    }
    $$('#navigation [data-route]').forEach(a => {
      const active = a.dataset.route === route.id;
      a.classList.toggle('active', active);
      if (active) { a.setAttribute('aria-current','page'); const details = a.closest('details'); if (details) details.open = true; }
      else a.removeAttribute('aria-current');
    });
    document.body.classList.remove('menu-open');
    $('#menu-toggle').setAttribute('aria-expanded','false');
    if (route.anchor) requestAnimationFrame(() => {
      const target = document.getElementById(route.anchor);
      if (target) target.scrollIntoView({block:'start'});
    });
    else if (samePage) window.scrollTo(0,0);
  }
  function renderRecord(r) {
    const isGuide = r.id.startsWith('guide/');
    const isHome = r.id === 'guide/welcome';
    const isAPI = r.id.startsWith('api/');
    document.title = `${r.title} · ALSA PCM`;
    main.className = (isHome ? 'home' : '') + (isAPI ? ' api-page' : '');
    const source = r.source ? `<a href="${escape(r.source)}">View upstream ↗</a>` : `<span>Learning material · not an API specification</span>`;
    main.innerHTML = `<div class="breadcrumbs"><a href="#/guide/welcome">Docs</a><span>/</span><span>${escape(r.category)}</span>${isAPI ? '<span>/</span><span class="accent">API</span>' : ''}</div>${!isHome ? `<div class="page-heading"><span class="content-tag ${isGuide ? 'editorial' : ''}">${isGuide ? 'FIELD GUIDE' : 'UPSTREAM REFERENCE'}</span><h1>${escape(r.title)}${isAPI && r.title.startsWith('snd_') && !r.title.endsWith('_t') ? '<span class="muted">()</span>' : ''}</h1><div class="page-meta">${source}${r.source ? `<span>Snapshot · ${upstream.fetched}</span>` : ''}</div></div>` : ''}<article class="prose">${r.html}</article>${r.source ? `<div class="source-note"><strong>From the ALSA project documentation</strong><p>Upstream wording, reformatted for this reader. <a href="${escape(r.source)}">Check the original page ↗</a></p></div>` : ''}${pagination(r)}<footer class="page-footer"><span>ALSA <b>/</b> field guide</span><a href="#/guide/how-to">Independent docs reader · About →</a></footer>`;
    const code = $('#playback-source'); if (code) code.textContent = upstream.example;
  }
  function pagination(r) {
    const sequence = r.id.startsWith('guide/') ? guides : r.id.startsWith('manual/') ? manual : [];
    const i = sequence.indexOf(r);
    if (i < 0) return `<div class="page-pagination"><a href="#/reference"><span>REFERENCE</span>← Browse all API entries</a></div>`;
    const previous = sequence[i-1], next = sequence[i+1];
    return `<div class="page-pagination">${previous ? `<a href="#/${previous.id}"><span>PREVIOUS</span>← ${escape(previous.title)}</a>` : '<div></div>'}${next ? `<a href="#/${next.id}"><span>UP NEXT</span>${escape(next.title)} →</a>` : '<div></div>'}</div>`;
  }
  function renderReference(query) {
    document.title = 'API reference · ALSA PCM';
    main.className = 'reference-page';
    main.innerHTML = `<div class="breadcrumbs"><a href="#/guide/welcome">Docs</a><span>/</span>Reference</div><div class="page-heading"><span class="content-tag">UPSTREAM REFERENCE</span><h1>The PCM API<span class="accent">.</span></h1><p class="lead">The exact calls, without the endless scroll.</p><div class="page-meta">${api.length} entries <span>${categories.length} topics · Snapshot ${upstream.fetched}</span></div></div><div class="reference-controls"><label for="api-filter">Filter symbols<input id="api-filter" type="search" placeholder="e.g. writei, hw_params, recover"></label><label for="category-filter">Topic<select id="category-filter"><option value="">All topics</option>${categories.map(c => `<option>${escape(c)}</option>`).join('')}</select></label></div><div class="result-count" id="api-count" role="status"></div><div id="api-list" class="api-list"></div>`;
    $('#api-filter').value = query.get('q') || '';
    $('#category-filter').value = categories.includes(query.get('category')) ? query.get('category') : '';
    const update = () => {
      const q = normalize($('#api-filter').value), category = $('#category-filter').value;
      const matches = api.filter(r => (!category || r.category === category) && q.split(' ').every(t => (r.searchTitle + ' ' + normalize(r.summary)).includes(t))).sort((a,b) => a.title.localeCompare(b.title));
      $('#api-count').textContent = `${matches.length} ${matches.length === 1 ? 'entry' : 'entries'}${category ? ' in ' + category : ' across all topics'}`;
      $('#api-list').innerHTML = matches.length ? matches.map(r => `<a href="#/${r.id}" class="api-row"><span class="function-icon">${r.title.startsWith('SND_') ? '#' : 'ƒ'}</span><div><code>${escape(r.title)}</code><p>${escape(r.summary)}</p><span>${escape(r.category)}</span></div><span class="row-arrow">↗</span></a>`).join('') : '<div class="empty-state"><h2>No matching entries</h2><p>Try a shorter name or select “All topics”.</p></div>';
      const params = new URLSearchParams(); if ($('#api-filter').value) params.set('q', $('#api-filter').value); if (category) params.set('category',category);
      history.replaceState(null,'','#/reference' + (params.size ? '?' + params : ''));
    };
    $('#api-filter').addEventListener('input', update); $('#category-filter').addEventListener('change', update); update();
  }
  function setupPage(id) {
    if (observer) observer.disconnect();
    const headings = $$('.prose h2, .prose h3, .memdoc > dl > dt', main).filter(h => !h.closest('a'));
    headings.forEach((h,i) => {
      if (!h.id) h.id = $('a[id]',h)?.id || 'section-' + i;
      // Move an upstream anchor to the heading, avoiding duplicate DOM IDs.
      const old = $('a[id]',h); if (old && old.id === h.id) old.removeAttribute('id');
      const link = document.createElement('a'); link.className = 'heading-anchor'; link.href = '#/' + id + '/' + h.id; link.textContent = '#'; link.setAttribute('aria-label','Link to ' + h.textContent); h.append(link);
    });
    $('#toc-links').innerHTML = headings.length ? headings.map(h => `<a href="#/${id}/${h.id}" data-heading="${h.id}" class="${h.tagName === 'H3' ? 'subheading' : ''}">${escape(h.textContent.replace(/#$/,''))}</a>`).join('') : `<a href="#/${id}">${id === 'reference' ? 'Browse & filter' : 'Overview'}</a>${id.startsWith('api/') ? '<a href="#/reference">All API entries →</a>' : ''}`;
    observer = new IntersectionObserver(entries => {
      const visible = entries.filter(e => e.isIntersecting).sort((a,b) => a.boundingClientRect.top-b.boundingClientRect.top)[0];
      if (visible) $$('#toc-links [data-heading]').forEach(a => a.classList.toggle('active', a.dataset.heading === visible.target.id));
    }, {rootMargin:'-90px 0px -60% 0px'});
    headings.forEach(h => observer.observe(h));
    $$('pre',main).forEach(pre => {
      const button = document.createElement('button'); button.className = 'copy-button'; button.textContent = 'Copy'; button.setAttribute('aria-label','Copy code');
      button.addEventListener('click', async () => {
        try { await navigator.clipboard.writeText($('code',pre)?.textContent || pre.textContent.replace(/Copy$/,'')); button.textContent = 'Copied!'; }
        catch { button.textContent = 'Select to copy'; }
        setTimeout(() => button.textContent = 'Copy',1800);
      });
      pre.append(button);
    });
    $$('table',main).forEach(table => { const wrapper=document.createElement('div'); wrapper.className='table-scroll'; table.before(wrapper); wrapper.append(table); });
  }
  const dialog = $('#search-dialog'), searchInput = $('#search-input');
  function search() {
    const q = normalize(searchInput.value), tokens = q.split(' ').filter(Boolean);
    const matches = tokens.length ? records.map(r => {
      if (!tokens.every(t => r.searchText.includes(t))) return null;
      let score = tokens.reduce((sum,t) => sum + (r.searchTitle.includes(t) ? 20 : 0),0);
      if (r.searchTitle === q || r.searchTitle === 'snd pcm ' + q) score += 100;
      if (r.searchTitle.startsWith(q)) score += 30;
      if (r.id.startsWith('guide/')) score += 5;
      return {r,score};
    }).filter(Boolean).sort((a,b) => b.score-a.score || a.r.title.localeCompare(b.r.title)).map(x => x.r) : [guides[1],guides[2],byId.get('api/snd_pcm_writei'),guides[3],byId.get('manual/pcm_params')].filter(Boolean);
    $('#search-status').textContent = q ? `${matches.length} results${matches.length > 60 ? ' · Showing the top 60; refine your search for more' : ''}` : 'SUGGESTED STARTING POINTS';
    $('#search-results').innerHTML = matches.length ? matches.slice(0,60).map(r => `<a class="search-result" href="#/${r.id}"><span class="search-kind">${r.id.startsWith('api/') ? 'ƒ' : '§'}</span><div><strong>${escape(r.title)}</strong><span>${escape(r.category)} · ${escape(r.summary.slice(0,155))}</span></div><span class="row-arrow">↵</span></a>`).join('') : `<div class="empty-state"><h2>No results for “${escape(searchInput.value)}”</h2><p>Try “frames”, “writei”, or “EPIPE”. Not all of ALSA is included in this snapshot.</p></div>`;
  }
  function openSearch() { dialog.showModal(); searchInput.value=''; search(); searchInput.focus(); }
  $('#search-open').addEventListener('click',openSearch);
  $('#search-close').addEventListener('click',() => dialog.close());
  dialog.addEventListener('click',e => { if (e.target === dialog) dialog.close(); if (e.target.closest('.search-result')) dialog.close(); });
  searchInput.addEventListener('input',search);
  dialog.addEventListener('keydown',e => {
    const links=$$('.search-result',dialog), index=links.indexOf(document.activeElement);
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (!links.length) return;
      const next = e.key === 'ArrowDown' ? Math.min(index+1,links.length-1) : index-1;
      if (next < 0) searchInput.focus(); else links[next].focus();
    } else if (e.key === 'Enter' && document.activeElement === searchInput && links[0]) { e.preventDefault(); links[0].click(); }
  });
  document.addEventListener('keydown',e => {
    const typing = e.target.matches('input,textarea,select,[contenteditable="true"]');
    if (((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') || (e.key === '/' && !typing && !dialog.open)) {
      e.preventDefault(); if (!dialog.open) openSearch();
    }
    if (e.key === 'Escape') { document.body.classList.remove('menu-open'); $('#menu-toggle').setAttribute('aria-expanded','false'); }
  });
  $('#menu-toggle').addEventListener('click',() => { const open=document.body.classList.toggle('menu-open'); $('#menu-toggle').setAttribute('aria-expanded',String(open)); });
  const media = matchMedia('(prefers-color-scheme: dark)');
  let storedTheme; try { storedTheme = localStorage.getItem('alsa-theme'); } catch {}
  function applyTheme(theme) { document.documentElement.dataset.theme=theme; $('#theme-toggle').setAttribute('aria-label',`Switch to ${theme === 'dark' ? 'light' : 'dark'} theme`); }
  applyTheme(storedTheme || (media.matches ? 'dark' : 'light'));
  $('#theme-toggle').addEventListener('click',() => { storedTheme=document.documentElement.dataset.theme==='dark'?'light':'dark'; applyTheme(storedTheme); try {localStorage.setItem('alsa-theme',storedTheme);} catch {} });
  media.addEventListener('change',e => {if (!storedTheme) applyTheme(e.matches?'dark':'light');});
  window.addEventListener('hashchange',render);
  render();
})();
