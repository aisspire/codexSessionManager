(function(){const i=document.createElement("link").relList;if(i&&i.supports&&i.supports("modulepreload"))return;for(const a of document.querySelectorAll('link[rel="modulepreload"]'))s(a);new MutationObserver(a=>{for(const r of a)if(r.type==="childList")for(const l of r.addedNodes)l.tagName==="LINK"&&l.rel==="modulepreload"&&s(l)}).observe(document,{childList:!0,subtree:!0});function n(a){const r={};return a.integrity&&(r.integrity=a.integrity),a.referrerPolicy&&(r.referrerPolicy=a.referrerPolicy),a.crossOrigin==="use-credentials"?r.credentials="include":a.crossOrigin==="anonymous"?r.credentials="omit":r.credentials="same-origin",r}function s(a){if(a.ep)return;a.ep=!0;const r=n(a);fetch(a.href,r)}})();async function f(e,i={},n){return window.__TAURI_INTERNALS__.invoke(e,i,n)}const B="未分组项目";function x(e){var s;const i=[],n=new Map;for(const a of e){const r=((s=a.project)==null?void 0:s.trim())||B;let l=n.get(r);l||(l={key:r,project:r,sessions:[]},n.set(r,l),i.push(l)),l.sessions.push(a)}return i}const $={"batch-edit":"批量编辑","session-management":"会话管理"},S=[{key:"select",label:"",width:46,minWidth:46,resizable:!1},{key:"session",label:"会话",width:360,minWidth:220,resizable:!0},{key:"provider",label:"提供方",width:130,minWidth:96,resizable:!0},{key:"model",label:"模型",width:160,minWidth:110,resizable:!0},{key:"state",label:"状态",width:112,minWidth:86,resizable:!0},{key:"updated",label:"更新时间",width:200,minWidth:150,resizable:!0}],g=()=>({editingField:"",draft:"",pendingId:"",pendingTitle:"",pendingProject:"",pendingProvider:""}),t={activePage:"batch-edit",profile:{codex_home:"~/.codex",path_maps:[]},filter:{archived:"all"},selectedEdit:{provider:"",project:"",titlePrefix:""},detailEdit:g(),sessions:[],selectedIds:new Set,activeId:"",detailOpen:!1,status:"就绪",columnWidths:S.map(e=>e.width),expandedProjects:new Set,hasInitializedProjectExpansion:!1},T=document.querySelector("#app");if(!T)throw new Error("missing app root");const C=T;function o(e={}){const i=e.preserveTableScroll?pe():void 0,n=x(t.sessions),s=t.sessions.find(a=>a.id===t.activeId);C.innerHTML=`
    <main class="shell">
      ${D()}
      <section class="workbench" aria-label="${d($[t.activePage])}">
        ${M()}
        ${N()}
        ${F()}
        ${K(n)}
        <div class="status">${d(t.status)}</div>
      </section>
      ${s&&t.detailOpen?U(s):""}
    </main>
  `,X(n),i&&fe(i)}function D(){return`
    <aside class="nav">
      <div class="brand">
        <span class="brand-mark">CSM</span>
        <span>Codex 会话管理</span>
      </div>
      <nav class="page-nav" aria-label="功能页面">
        ${k("batch-edit")}
        ${k("session-management")}
      </nav>
    </aside>
  `}function k(e){return`
    <button class="page-nav-button ${t.activePage===e?"selected":""}" data-page="${e}">
      ${d($[e])}
    </button>
  `}function M(){const e=t.activePage==="batch-edit"?"批量修改已选会话的名称前缀、提供方和项目路径。":"备份、归档、恢复、置顶或删除已选会话。";return`
    <header class="page-header">
      <div>
        <h1>${d($[t.activePage])}</h1>
        <p>${d(e)}</p>
      </div>
      <div class="page-count">
        <strong>${t.sessions.length}</strong>
        <span>会话</span>
        <strong>${t.selectedIds.size}</strong>
        <span>已选</span>
      </div>
    </header>
  `}function N(){return`
    <section class="toolbar filter-toolbar" aria-label="搜索筛选">
      <label>Codex 主目录<input id="codex-home" value="${d(t.profile.codex_home)}" /></label>
      <label>项目<input id="project" value="${d(t.filter.project??"")}" /></label>
      <label>提供方<input id="provider" value="${d(t.filter.provider??"")}" /></label>
      <label>模型<input id="model" value="${d(t.filter.model??"")}" /></label>
      <label>来源<input id="source" value="${d(t.filter.source??"")}" /></label>
      <label>搜索<input id="search" value="${d(t.filter.search??"")}" /></label>
      <div class="segmented" role="group" aria-label="归档状态">
        ${y("all","全部")}
        ${y("active","活动")}
        ${y("archived","已归档")}
      </div>
      <button id="refresh" class="primary">刷新</button>
    </section>
  `}function F(){return t.activePage==="batch-edit"?R():V()}function R(){return`
    <section class="toolbar action-toolbar" aria-label="批量编辑操作">
      <label>会话名前缀<input id="edit-title-prefix" placeholder="多选时生成 前缀(1)" value="${d(t.selectedEdit.titlePrefix)}" /></label>
      <label>提供方<input id="edit-provider" placeholder="留空则不改" value="${d(t.selectedEdit.provider)}" /></label>
      <label>项目路径<input id="edit-project" placeholder="留空则不改" value="${d(t.selectedEdit.project)}" /></label>
      <div class="action-buttons">
        <button id="preview-selected-edit">预览</button>
        <button id="apply-selected-edit" class="primary">应用</button>
      </div>
    </section>
  `}function V(){return`
    <section class="toolbar action-toolbar management-toolbar" aria-label="会话管理操作">
      <button id="backup">备份</button>
      <button id="archive">归档</button>
      <button id="restore">恢复</button>
      <button id="refresh-time">置顶</button>
      <button id="delete" class="danger">删除</button>
    </section>
  `}function y(e,i){return`<button data-archived="${e}" class="${t.filter.archived===e?"selected":""}">${i}</button>`}function K(e){return`
    <section class="table-shell" aria-label="会话列表">
      <div class="table" style="${oe()}">
        ${G()}
        ${e.length?e.map(i=>H(i)).join(""):'<div class="empty-list">没有匹配的会话</div>'}
      </div>
    </section>
  `}function G(){return`<div class="row header">${S.map((i,n)=>i.key==="select"?`
      <span class="header-cell select-header-cell">
        <input id="select-all" type="checkbox" aria-label="全选当前列表" ${A()?"checked":""} />
      </span>
    `:`
      <span class="header-cell">
        <span class="header-label">${d(i.label)}</span>
        ${i.resizable?`<span class="resize-handle" data-resize-column="${n}" role="separator" aria-label="调整${d(i.label)}列宽"></span>`:""}
      </span>
    `).join("")}</div>`}function H(e){const i=t.expandedProjects.has(e.key),n=e.sessions.filter(a=>t.selectedIds.has(a.id)).length,s=e.sessions.length>0&&n===e.sessions.length;return`
    <section class="project-group" data-project-group="${d(e.key)}">
      <div class="project-group-header">
        <button class="project-toggle" data-toggle-project="${d(e.key)}" aria-expanded="${i}">
          <span class="chevron">${i?"▾":"▸"}</span>
          <span class="project-title">${d(e.project)}</span>
          <span class="project-meta">${e.sessions.length} 个会话 · 已选 ${n}</span>
        </button>
        <label class="group-select">
          <input type="checkbox" data-select-project="${d(e.key)}" ${s?"checked":""} />
          组内全选
        </label>
      </div>
      ${i?e.sessions.map(J).join(""):""}
    </section>
  `}function J(e){const i=t.selectedIds.has(e.id);return`
    <button class="row session-row ${t.activeId===e.id&&t.detailOpen?"active":""}" data-open="${d(e.id)}">
      <input type="checkbox" data-select="${d(e.id)}" ${i?"checked":""} />
      <span class="session-title">${d(P(e))}</span>
      <span>${d(e.provider||"")}</span>
      <span>${d(e.model||"")}</span>
      <span>${e.archived?"已归档":"活动"}</span>
      <span>${d(e.updated_at||"")}</span>
    </button>
  `}function U(e){const i=P(e),n=p(e,"title"),s=z(e);return`
    <div class="drawer-backdrop" data-close-detail></div>
    <aside class="detail-drawer" aria-label="会话详情">
      <div class="drawer-top">
        <span>会话详情</span>
        <button class="icon-button" data-close-detail title="关闭详情">×</button>
      </div>
      <div class="detail-title-row">
        ${t.detailEdit.editingField==="title"&&t.detailEdit.pendingId===e.id?`<input id="detail-edit-input" class="detail-title-input" value="${d(t.detailEdit.draft)}" />`:`<h2>${d(n||i)}</h2><button data-detail-edit="title" class="icon-button" title="重命名会话">✎</button>`}
      </div>
      <dl>
        <dt>ID</dt><dd>${d(e.id)}</dd>
        ${L(e,"项目","project")}
        ${L(e,"提供方","provider")}
        <dt>模型</dt><dd>${d(e.model||"")}</dd>
        <dt>来源</dt><dd>${d(e.source||"")}</dd>
        <dt>会话文件</dt><dd>${d(e.rollout_path||"")}</dd>
        <dt>会话索引</dt><dd>${e.in_session_index?"存在":"缺失"}</dd>
      </dl>
      <div class="detail-actions">
        <button id="save-detail-title" class="primary" ${s?"":"disabled"}>保存</button>
        <button data-single-command="refresh_session_updated_at">置顶</button>
        <button data-single="archive">归档</button>
        <button data-single="restore">恢复</button>
        <button data-single="delete" class="danger">删除</button>
      </div>
    </aside>
  `}function X(e){var i,n,s,a,r,l,c,m;Q(),Y(),Z(),ee(),te(e),ie(),ne(),ve(),(i=document.querySelector("#refresh"))==null||i.addEventListener("click",j),(n=document.querySelector("#preview-selected-edit"))==null||n.addEventListener("click",()=>I(!1)),(s=document.querySelector("#apply-selected-edit"))==null||s.addEventListener("click",()=>I(!0)),(a=document.querySelector("#archive"))==null||a.addEventListener("click",()=>h("archive_sessions")),(r=document.querySelector("#restore"))==null||r.addEventListener("click",()=>h("restore_sessions")),(l=document.querySelector("#refresh-time"))==null||l.addEventListener("click",()=>h("refresh_session_updated_at")),(c=document.querySelector("#delete"))==null||c.addEventListener("click",()=>h("delete_sessions")),(m=document.querySelector("#backup"))==null||m.addEventListener("click",le)}function Q(){document.querySelectorAll("[data-page]").forEach(e=>{e.addEventListener("click",()=>{t.activePage=e.dataset.page,o({preserveTableScroll:!0})})})}function Y(){u("codex-home",e=>t.profile.codex_home=e),u("project",e=>t.filter.project=v(e)),u("provider",e=>t.filter.provider=v(e)),u("model",e=>t.filter.model=v(e)),u("source",e=>t.filter.source=v(e)),u("search",e=>t.filter.search=v(e)),document.querySelectorAll("[data-archived]").forEach(e=>{e.addEventListener("click",()=>{t.filter.archived=e.dataset.archived,j()})})}function Z(){u("edit-title-prefix",e=>t.selectedEdit.titlePrefix=e),u("edit-provider",e=>t.selectedEdit.provider=e),u("edit-project",e=>t.selectedEdit.project=e)}function ee(){const e=document.querySelector("#select-all");e&&(e.indeterminate=ue()&&!A(),e.addEventListener("click",i=>i.stopPropagation()),e.addEventListener("change",()=>{e.checked?t.sessions.forEach(i=>t.selectedIds.add(i.id)):t.sessions.forEach(i=>t.selectedIds.delete(i.id)),o({preserveTableScroll:!0})}))}function te(e){const i=new Map(e.map(n=>[n.key,n]));document.querySelectorAll("[data-toggle-project]").forEach(n=>{n.addEventListener("click",()=>{const s=n.dataset.toggleProject||"";t.expandedProjects.has(s)?t.expandedProjects.delete(s):t.expandedProjects.add(s),o({preserveTableScroll:!0})})}),document.querySelectorAll("[data-select-project]").forEach(n=>{const s=i.get(n.dataset.selectProject||"");if(!s)return;const a=s.sessions.filter(r=>t.selectedIds.has(r.id)).length;n.indeterminate=a>0&&a<s.sessions.length,n.addEventListener("click",r=>r.stopPropagation()),n.addEventListener("change",()=>{for(const r of s.sessions)n.checked?t.selectedIds.add(r.id):t.selectedIds.delete(r.id);o({preserveTableScroll:!0})})})}function ie(){document.querySelectorAll("[data-open]").forEach(e=>{e.addEventListener("click",()=>{t.activeId=e.dataset.open||"",t.detailOpen=!0,t.detailEdit=g(),o({preserveTableScroll:!0})})}),document.querySelectorAll("[data-select]").forEach(e=>{e.addEventListener("click",i=>{i.stopPropagation();const n=e.dataset.select||"";e.checked?t.selectedIds.add(n):t.selectedIds.delete(n),o({preserveTableScroll:!0})})})}function ne(){var i;document.querySelectorAll("[data-close-detail]").forEach(n=>{n.addEventListener("click",()=>{t.detailOpen=!1,t.detailEdit=g(),o({preserveTableScroll:!0})})}),document.querySelectorAll("[data-detail-edit]").forEach(n=>{n.addEventListener("click",()=>se(n.dataset.detailEdit))}),(i=document.querySelector("#save-detail-title"))==null||i.addEventListener("click",ae);const e=document.querySelector("#detail-edit-input");e&&(e.focus(),e.select(),e.addEventListener("input",()=>{t.detailEdit.draft=e.value}),e.addEventListener("keydown",n=>{n.key==="Enter"&&(_(),o({preserveTableScroll:!0})),n.key==="Escape"&&(t.detailEdit.editingField="",o({preserveTableScroll:!0}))}),e.addEventListener("blur",()=>{_(),window.setTimeout(()=>o({preserveTableScroll:!0}),0)})),document.querySelectorAll("[data-single], [data-single-command]").forEach(n=>{const s=n.dataset.singleCommand||`${n.dataset.single}_sessions`;n.addEventListener("click",()=>q(s,[t.activeId]))})}function u(e,i){var n;(n=document.querySelector(`#${e}`))==null||n.addEventListener("change",s=>{i(s.target.value)})}async function j(){await b(async()=>{var e;t.sessions=await f("list_sessions",{profile:t.profile,filter:t.filter}),t.selectedIds.clear(),t.activeId=((e=t.sessions[0])==null?void 0:e.id)||"",t.detailOpen=!1,w(),t.status="已加载会话"})}function w(){const e=x(t.sessions),i=new Set(e.map(n=>n.key));if(!t.hasInitializedProjectExpansion){t.expandedProjects=i,t.hasInitializedProjectExpansion=!0;return}for(const n of t.expandedProjects)i.has(n)||t.expandedProjects.delete(n);for(const n of i)t.expandedProjects.has(n)||t.expandedProjects.add(n)}async function h(e){await q(e,[...t.selectedIds])}async function q(e,i){if(i.length===0){t.status="请至少选择一个会话",o();return}await b(async()=>{const n=await f(e,{profile:t.profile,ids:i,apply:!0});t.status=JSON.stringify(n),await j()})}async function I(e){const i=[...t.selectedIds],n=t.selectedEdit.provider.trim(),s=t.selectedEdit.project.trim(),a=t.selectedEdit.titlePrefix.trim();if(i.length===0){t.status="请至少选择一个会话",o();return}if(!n&&!s&&!a){t.status="请填写会话名前缀、提供方或项目路径",o({preserveTableScroll:!0});return}e&&!window.confirm(`将修改 ${i.length} 个已选会话，并在写入前创建备份。继续？`)||await b(async()=>{var l;const r=await f("edit_selected_sessions",{profile:t.profile,ids:i,edit:{provider:n||null,project:s||null,titlePrefix:a||null},apply:e});e&&(t.sessions=await f("list_sessions",{profile:t.profile,filter:t.filter}),t.selectedIds.clear(),t.activeId=((l=t.sessions[0])==null?void 0:l.id)||"",t.detailOpen=!1,w()),t.status=O(r)})}function L(e,i,n){const s=t.detailEdit.editingField===n&&t.detailEdit.pendingId===e.id,a=s?t.detailEdit.draft:de(e,n);return`
    <dt>${d(i)}</dt>
    <dd class="detail-editable-value">
      ${s?`<input id="detail-edit-input" class="detail-inline-input" value="${d(a)}" />`:`<span>${d(a)}</span><button data-detail-edit="${n}" class="icon-button" title="修改${d(i)}">✎</button>`}
    </dd>
  `}function se(e){const i=t.sessions.find(n=>n.id===t.activeId);i&&(t.detailEdit={...t.detailEdit,editingField:e,draft:p(i,e)||E(i,e),pendingId:i.id},o({preserveTableScroll:!0}))}function _(){const e=t.sessions.find(s=>s.id===t.activeId),i=t.detailEdit.editingField;if(!e||!i||t.detailEdit.pendingId!==e.id)return;const n=t.detailEdit.draft.trim()||E(e,i);t.detailEdit.editingField="",re(i,n)}async function ae(){const e=t.sessions.find(a=>a.id===t.activeId);if(!e||!z(e))return;const i=p(e,"title"),n=p(e,"project"),s=p(e,"provider");await b(async()=>{var l;const a=await f("edit_selected_sessions",{profile:t.profile,ids:[e.id],edit:{title:i||null,project:n||null,provider:s||null},apply:!0}),r=e.id;t.sessions=await f("list_sessions",{profile:t.profile,filter:t.filter}),t.activeId=t.sessions.some(c=>c.id===r)?r:((l=t.sessions[0])==null?void 0:l.id)||"",t.detailOpen=!!t.activeId,t.detailEdit=g(),w(),t.status=O(a)})}function P(e){return e.title||e.first_user_message||e.id}function E(e,i){return i==="title"?P(e):i==="project"?e.project||"":e.provider||""}function de(e,i){return p(e,i)||E(e,i)}function p(e,i){return t.detailEdit.pendingId!==e.id?"":i==="title"?t.detailEdit.pendingTitle.trim():i==="project"?t.detailEdit.pendingProject.trim():t.detailEdit.pendingProvider.trim()}function re(e,i){e==="title"?t.detailEdit.pendingTitle=i:e==="project"?t.detailEdit.pendingProject=i:t.detailEdit.pendingProvider=i}function z(e){return["title","project","provider"].some(i=>{const n=p(e,i);return n.length>0&&n!==E(e,i)})}async function le(){await b(async()=>{const e=await f("create_backup",{profile:t.profile,includeSessions:!1});t.status=JSON.stringify(e)})}async function b(e){try{t.status="正在处理...",o(),await e()}catch(i){t.status=String(i)}finally{o()}}function O(e){const i=e.backup_dir?` · 备份 ${e.backup_dir}`:"";return`${e.action} · ${e.applied?"已应用":"预览"} · SQLite ${e.sqlite_rows} 行 · JSONL ${e.jsonl_files} 个 · 索引 ${e.index_entries} 条${i}`}function oe(){const e=t.columnWidths.map(n=>`${n}px`).join(" "),i=t.columnWidths.reduce((n,s)=>n+s,0);return`--session-grid: ${e}; --session-table-width: ${i}px;`}function ce(){const e=document.querySelector(".table");if(!e)return;const i=t.columnWidths.map(s=>`${s}px`).join(" "),n=t.columnWidths.reduce((s,a)=>s+a,0);e.style.setProperty("--session-grid",i),e.style.setProperty("--session-table-width",`${n}px`)}function A(){return t.sessions.length>0&&t.sessions.every(e=>t.selectedIds.has(e.id))}function ue(){return t.sessions.some(e=>t.selectedIds.has(e.id))}function pe(){const e=document.querySelector(".table");return{left:(e==null?void 0:e.scrollLeft)??0,top:(e==null?void 0:e.scrollTop)??0}}function fe(e){const i=document.querySelector(".table");i&&(i.scrollLeft=e.left,i.scrollTop=e.top)}function ve(){document.querySelectorAll("[data-resize-column]").forEach(e=>{e.addEventListener("pointerdown",i=>{i.preventDefault();const n=Number(e.dataset.resizeColumn),s=S[n];if(!s)return;const a=i.clientX,r=t.columnWidths[n];document.body.classList.add("resizing-column");const l=m=>{const W=Math.max(s.minWidth,r+m.clientX-a);t.columnWidths[n]=Math.round(W),ce()},c=()=>{document.body.classList.remove("resizing-column"),document.removeEventListener("pointermove",l),document.removeEventListener("pointerup",c),document.removeEventListener("pointercancel",c)};document.addEventListener("pointermove",l),document.addEventListener("pointerup",c),document.addEventListener("pointercancel",c)})})}function v(e){const i=e.trim();return i||void 0}function d(e){return e.replace(/[&<>"']/g,i=>({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#039;"})[i])}o();
