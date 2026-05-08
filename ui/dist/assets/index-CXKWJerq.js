(function(){const i=document.createElement("link").relList;if(i&&i.supports&&i.supports("modulepreload"))return;for(const n of document.querySelectorAll('link[rel="modulepreload"]'))l(n);new MutationObserver(n=>{for(const a of n)if(a.type==="childList")for(const o of a.addedNodes)o.tagName==="LINK"&&o.rel==="modulepreload"&&l(o)}).observe(document,{childList:!0,subtree:!0});function d(n){const a={};return n.integrity&&(a.integrity=n.integrity),n.referrerPolicy&&(a.referrerPolicy=n.referrerPolicy),n.crossOrigin==="use-credentials"?a.credentials="include":n.crossOrigin==="anonymous"?a.credentials="omit":a.credentials="same-origin",a}function l(n){if(n.ep)return;n.ep=!0;const a=d(n);fetch(n.href,a)}})();async function v(e,i={},d){return window.__TAURI_INTERNALS__.invoke(e,i,d)}const I=[{key:"select",label:"",width:42,minWidth:42,resizable:!1},{key:"session",label:"会话",width:280,minWidth:180,resizable:!0},{key:"project",label:"项目",width:220,minWidth:140,resizable:!0},{key:"provider",label:"提供方",width:120,minWidth:90,resizable:!0},{key:"model",label:"模型",width:150,minWidth:100,resizable:!0},{key:"state",label:"状态",width:110,minWidth:86,resizable:!0},{key:"updated",label:"更新时间",width:190,minWidth:140,resizable:!0}],t={profile:{codex_home:"~/.codex",path_maps:[]},filter:{archived:"all"},selectedEdit:{provider:"",project:"",titlePrefix:""},detailEdit:{editingField:"",draft:"",pendingId:"",pendingTitle:"",pendingProject:"",pendingProvider:""},sessions:[],selectedIds:new Set,activeId:"",status:"就绪",columnWidths:I.map(e=>e.width)},q=document.querySelector("#app");if(!q)throw new Error("missing app root");const D=q;function c(e={}){const i=e.preserveTableScroll?K():void 0,d=t.sessions.find(l=>l.id===t.activeId);D.innerHTML=`
    <main class="shell">
      <aside class="filters">
        <div class="brand">Codex 会话管理</div>
        <label>Codex 主目录<input id="codex-home" value="${s(t.profile.codex_home)}" /></label>
        <label>项目<input id="project" value="${s(t.filter.project??"")}" /></label>
        <label>提供方<input id="provider" value="${s(t.filter.provider??"")}" /></label>
        <label>模型<input id="model" value="${s(t.filter.model??"")}" /></label>
        <label>来源<input id="source" value="${s(t.filter.source??"")}" /></label>
        <label>搜索<input id="search" value="${s(t.filter.search??"")}" /></label>
        <div class="segmented" role="group">
          ${S("all","全部")}
          ${S("active","活动")}
          ${S("archived","已归档")}
        </div>
        <button id="refresh" class="primary">刷新</button>
        <div class="edit-panel">
          <div class="edit-title">修改已选</div>
          <label>会话名前缀<input id="edit-title-prefix" placeholder="多选时生成 前缀(1)" value="${s(t.selectedEdit.titlePrefix)}" /></label>
          <label>提供方<input id="edit-provider" placeholder="留空则不改" value="${s(t.selectedEdit.provider)}" /></label>
          <label>项目路径<input id="edit-project" placeholder="留空则不改" value="${s(t.selectedEdit.project)}" /></label>
          <div class="edit-actions">
            <button id="preview-selected-edit">预览</button>
            <button id="apply-selected-edit" class="primary">应用</button>
          </div>
        </div>
      </aside>
      <section class="workbench">
        <div class="toolbar">
          <div>${t.sessions.length} 个会话 · 已选 ${t.selectedIds.size} 个</div>
          <button id="backup" title="创建备份">备份</button>
          <button id="archive" title="归档已选会话">归档</button>
          <button id="restore" title="恢复已选会话">恢复</button>
          <button id="refresh-time" title="将已选会话更新时间改为当前时间">置顶</button>
          <button id="delete" class="danger" title="将已选会话移入回收站">删除</button>
        </div>
        <div class="table" style="${U()}">
          ${N()}
          ${t.sessions.map(O).join("")}
        </div>
        <div class="status">${s(t.status)}</div>
      </section>
      <aside class="details">
        ${d?F(d):'<div class="empty">请选择一个会话</div>'}
      </aside>
    </main>
  `,C(),i&&Q(i)}function S(e,i){return`<button data-archived="${e}" class="${t.filter.archived===e?"selected":""}">${i}</button>`}function N(){return`<div class="row header">${I.map((i,d)=>i.key==="select"?`
      <span class="header-cell select-header-cell">
        <input id="select-all" type="checkbox" aria-label="全选当前列表" ${A()?"checked":""} />
      </span>
    `:`
      <span class="header-cell">
        <span class="header-label">${s(i.label)}</span>
        ${i.resizable?`<span class="resize-handle" data-resize-column="${d}" role="separator" aria-label="调整${s(i.label)}列宽"></span>`:""}
      </span>
    `).join("")}</div>`}function O(e){const i=t.selectedIds.has(e.id);return`
    <button class="row ${t.activeId===e.id?"active":""}" data-open="${s(e.id)}">
      <input type="checkbox" data-select="${s(e.id)}" ${i?"checked":""} />
      <span>${s(e.title||e.first_user_message||e.id)}</span>
      <span>${s(e.project||"")}</span>
      <span>${s(e.provider||"")}</span>
      <span>${s(e.model||"")}</span>
      <span>${e.archived?"已归档":"活动"}</span>
      <span>${s(e.updated_at||"")}</span>
    </button>
  `}function F(e){const i=x(e),d=f(e,"title"),l=z(e);return`
    <div class="detail-title-row">
      ${t.detailEdit.editingField==="title"&&t.detailEdit.pendingId===e.id?`<input id="detail-edit-input" class="detail-title-input" value="${s(t.detailEdit.draft)}" />`:`<h2>${s(d||i)}</h2><button data-detail-edit="title" class="icon-button" title="重命名会话">✎</button>`}
    </div>
    <dl>
      <dt>ID</dt><dd>${s(e.id)}</dd>
      ${j(e,"项目","project")}
      ${j(e,"提供方","provider")}
      <dt>模型</dt><dd>${s(e.model||"")}</dd>
      <dt>来源</dt><dd>${s(e.source||"")}</dd>
      <dt>会话文件</dt><dd>${s(e.rollout_path||"")}</dd>
      <dt>会话索引</dt><dd>${e.in_session_index?"存在":"缺失"}</dd>
    </dl>
    <div class="detail-actions">
      <button id="save-detail-title" class="primary" ${l?"":"disabled"}>保存</button>
      <button data-single-command="refresh_session_updated_at">置顶</button>
      <button data-single="archive">归档</button>
      <button data-single="restore">恢复</button>
      <button data-single="delete" class="danger">删除</button>
    </div>
  `}function C(){var d,l,n,a,o,u,h,E,L;p("codex-home",r=>t.profile.codex_home=r),p("project",r=>t.filter.project=m(r)),p("provider",r=>t.filter.provider=m(r)),p("model",r=>t.filter.model=m(r)),p("source",r=>t.filter.source=m(r)),p("search",r=>t.filter.search=m(r)),p("edit-title-prefix",r=>t.selectedEdit.titlePrefix=r),p("edit-provider",r=>t.selectedEdit.provider=r),p("edit-project",r=>t.selectedEdit.project=r),(d=document.querySelector("#refresh"))==null||d.addEventListener("click",w);const e=document.querySelector("#select-all");e&&(e.indeterminate=B()&&!A(),e.addEventListener("click",r=>r.stopPropagation()),e.addEventListener("change",()=>{e.checked?t.sessions.forEach(r=>t.selectedIds.add(r.id)):t.sessions.forEach(r=>t.selectedIds.delete(r.id)),c({preserveTableScroll:!0})})),(l=document.querySelector("#preview-selected-edit"))==null||l.addEventListener("click",()=>k(!1)),(n=document.querySelector("#apply-selected-edit"))==null||n.addEventListener("click",()=>k(!0)),(a=document.querySelector("#archive"))==null||a.addEventListener("click",()=>y("archive_sessions")),(o=document.querySelector("#restore"))==null||o.addEventListener("click",()=>y("restore_sessions")),(u=document.querySelector("#refresh-time"))==null||u.addEventListener("click",()=>y("refresh_session_updated_at")),(h=document.querySelector("#delete"))==null||h.addEventListener("click",()=>y("delete_sessions")),(E=document.querySelector("#backup"))==null||E.addEventListener("click",J),document.querySelectorAll("[data-detail-edit]").forEach(r=>{r.addEventListener("click",()=>R(r.dataset.detailEdit))}),(L=document.querySelector("#save-detail-title"))==null||L.addEventListener("click",M);const i=document.querySelector("#detail-edit-input");i&&(i.focus(),i.select(),i.addEventListener("input",()=>{t.detailEdit.draft=i.value}),i.addEventListener("keydown",r=>{r.key==="Enter"&&(P(),c({preserveTableScroll:!0}))}),i.addEventListener("blur",()=>{P(),window.setTimeout(()=>c({preserveTableScroll:!0}),0)})),document.querySelectorAll("[data-archived]").forEach(r=>{r.addEventListener("click",()=>{t.filter.archived=r.dataset.archived,w()})}),document.querySelectorAll("[data-open]").forEach(r=>{r.addEventListener("click",()=>{t.activeId=r.dataset.open||"",c({preserveTableScroll:!0})})}),document.querySelectorAll("[data-select]").forEach(r=>{r.addEventListener("click",$=>{$.stopPropagation();const _=r.dataset.select||"";r.checked?t.selectedIds.add(_):t.selectedIds.delete(_),c({preserveTableScroll:!0})})}),document.querySelectorAll("[data-single], [data-single-command]").forEach(r=>{const $=r.dataset.singleCommand||`${r.dataset.single}_sessions`;r.addEventListener("click",()=>T($,[t.activeId]))}),G()}function p(e,i){var d;(d=document.querySelector(`#${e}`))==null||d.addEventListener("change",l=>{i(l.target.value)})}async function w(){await b(async()=>{var e;t.sessions=await v("list_sessions",{profile:t.profile,filter:t.filter}),t.selectedIds.clear(),t.activeId=((e=t.sessions[0])==null?void 0:e.id)||"",t.status="已加载会话"})}async function y(e){await T(e,[...t.selectedIds])}async function T(e,i){if(i.length===0){t.status="请至少选择一个会话",c();return}await b(async()=>{const d=await v(e,{profile:t.profile,ids:i,apply:!0});t.status=JSON.stringify(d),await w()})}async function k(e){const i=[...t.selectedIds],d=t.selectedEdit.provider.trim(),l=t.selectedEdit.project.trim(),n=t.selectedEdit.titlePrefix.trim();if(i.length===0){t.status="请至少选择一个会话",c();return}if(!d&&!l&&!n){t.status="请填写会话名前缀、提供方或项目路径",c({preserveTableScroll:!0});return}e&&!window.confirm(`将修改 ${i.length} 个已选会话，并在写入前创建备份。继续？`)||await b(async()=>{var o;const a=await v("edit_selected_sessions",{profile:t.profile,ids:i,edit:{provider:d||null,project:l||null,titlePrefix:n||null},apply:e});e&&(t.sessions=await v("list_sessions",{profile:t.profile,filter:t.filter}),t.selectedIds.clear(),t.activeId=((o=t.sessions[0])==null?void 0:o.id)||""),t.status=W(a)})}function j(e,i,d){const l=t.detailEdit.editingField===d&&t.detailEdit.pendingId===e.id,n=l?t.detailEdit.draft:V(e,d);return`
    <dt>${s(i)}</dt>
    <dd class="detail-editable-value">
      ${l?`<input id="detail-edit-input" class="detail-inline-input" value="${s(n)}" />`:`<span>${s(n)}</span><button data-detail-edit="${d}" class="icon-button" title="修改${s(i)}">✎</button>`}
    </dd>
  `}function R(e){const i=t.sessions.find(d=>d.id===t.activeId);i&&(t.detailEdit={...t.detailEdit,editingField:e,draft:f(i,e)||g(i,e),pendingId:i.id},c({preserveTableScroll:!0}))}function P(){const e=t.sessions.find(l=>l.id===t.activeId),i=t.detailEdit.editingField;if(!e||!i||t.detailEdit.pendingId!==e.id)return;const d=t.detailEdit.draft.trim()||g(e,i);t.detailEdit.editingField="",H(i,d)}async function M(){const e=t.sessions.find(n=>n.id===t.activeId);if(!e||!z(e))return;const i=f(e,"title"),d=f(e,"project"),l=f(e,"provider");await b(async()=>{var o;const n=await v("edit_selected_sessions",{profile:t.profile,ids:[e.id],edit:{title:i||null,project:d||null,provider:l||null},apply:!0}),a=e.id;t.sessions=await v("list_sessions",{profile:t.profile,filter:t.filter}),t.activeId=t.sessions.some(u=>u.id===a)?a:((o=t.sessions[0])==null?void 0:o.id)||"",t.detailEdit={editingField:"",draft:"",pendingId:"",pendingTitle:"",pendingProject:"",pendingProvider:""},t.status=W(n)})}function x(e){return e.title||e.first_user_message||e.id}function g(e,i){return i==="title"?x(e):i==="project"?e.project||"":e.provider||""}function V(e,i){return f(e,i)||g(e,i)}function f(e,i){return t.detailEdit.pendingId!==e.id?"":i==="title"?t.detailEdit.pendingTitle.trim():i==="project"?t.detailEdit.pendingProject.trim():t.detailEdit.pendingProvider.trim()}function H(e,i){e==="title"?t.detailEdit.pendingTitle=i:e==="project"?t.detailEdit.pendingProject=i:t.detailEdit.pendingProvider=i}function z(e){return["title","project","provider"].some(i=>{const d=f(e,i);return d.length>0&&d!==g(e,i)})}async function J(){await b(async()=>{const e=await v("create_backup",{profile:t.profile,includeSessions:!1});t.status=JSON.stringify(e)})}async function b(e){try{t.status="正在处理...",c(),await e()}catch(i){t.status=String(i)}finally{c()}}function W(e){const i=e.backup_dir?` · 备份 ${e.backup_dir}`:"";return`${e.action} · ${e.applied?"已应用":"预览"} · SQLite ${e.sqlite_rows} 行 · JSONL ${e.jsonl_files} 个${i}`}function U(){const e=t.columnWidths.map(d=>`${d}px`).join(" "),i=t.columnWidths.reduce((d,l)=>d+l,0);return`--session-grid: ${e}; --session-table-width: ${i}px;`}function X(){const e=document.querySelector(".table");if(!e)return;const i=t.columnWidths.map(l=>`${l}px`).join(" "),d=t.columnWidths.reduce((l,n)=>l+n,0);e.style.setProperty("--session-grid",i),e.style.setProperty("--session-table-width",`${d}px`)}function A(){return t.sessions.length>0&&t.sessions.every(e=>t.selectedIds.has(e.id))}function B(){return t.sessions.some(e=>t.selectedIds.has(e.id))}function K(){const e=document.querySelector(".table");return{left:(e==null?void 0:e.scrollLeft)??0,top:(e==null?void 0:e.scrollTop)??0}}function Q(e){const i=document.querySelector(".table");i&&(i.scrollLeft=e.left,i.scrollTop=e.top)}function G(){document.querySelectorAll("[data-resize-column]").forEach(e=>{e.addEventListener("pointerdown",i=>{i.preventDefault();const d=Number(e.dataset.resizeColumn),l=I[d];if(!l)return;const n=i.clientX,a=t.columnWidths[d];document.body.classList.add("resizing-column");const o=h=>{const E=Math.max(l.minWidth,a+h.clientX-n);t.columnWidths[d]=Math.round(E),X()},u=()=>{document.body.classList.remove("resizing-column"),document.removeEventListener("pointermove",o),document.removeEventListener("pointerup",u),document.removeEventListener("pointercancel",u)};document.addEventListener("pointermove",o),document.addEventListener("pointerup",u),document.addEventListener("pointercancel",u)})})}function m(e){const i=e.trim();return i||void 0}function s(e){return e.replace(/[&<>"']/g,i=>({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#039;"})[i])}c();
