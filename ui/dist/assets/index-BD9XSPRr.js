(function(){const i=document.createElement("link").relList;if(i&&i.supports&&i.supports("modulepreload"))return;for(const n of document.querySelectorAll('link[rel="modulepreload"]'))l(n);new MutationObserver(n=>{for(const o of n)if(o.type==="childList")for(const a of o.addedNodes)a.tagName==="LINK"&&a.rel==="modulepreload"&&l(a)}).observe(document,{childList:!0,subtree:!0});function s(n){const o={};return n.integrity&&(o.integrity=n.integrity),n.referrerPolicy&&(o.referrerPolicy=n.referrerPolicy),n.crossOrigin==="use-credentials"?o.credentials="include":n.crossOrigin==="anonymous"?o.credentials="omit":o.credentials="same-origin",o}function l(n){if(n.ep)return;n.ep=!0;const o=s(n);fetch(n.href,o)}})();async function f(e,i={},s){return window.__TAURI_INTERNALS__.invoke(e,i,s)}const S=[{key:"select",label:"",width:42,minWidth:42,resizable:!1},{key:"session",label:"会话",width:280,minWidth:180,resizable:!0},{key:"project",label:"项目",width:220,minWidth:140,resizable:!0},{key:"provider",label:"提供方",width:120,minWidth:90,resizable:!0},{key:"model",label:"模型",width:150,minWidth:100,resizable:!0},{key:"state",label:"状态",width:110,minWidth:86,resizable:!0},{key:"updated",label:"更新时间",width:190,minWidth:140,resizable:!0}],t={profile:{codex_home:"~/.codex",path_maps:[]},filter:{archived:"all"},selectedEdit:{provider:"",project:""},sessions:[],selectedIds:new Set,activeId:"",status:"就绪",columnWidths:S.map(e=>e.width)},g=document.querySelector("#app");if(!g)throw new Error("missing app root");const I=g;function c(e={}){const i=e.preserveTableScroll?M():void 0,s=t.sessions.find(l=>l.id===t.activeId);I.innerHTML=`
    <main class="shell">
      <aside class="filters">
        <div class="brand">Codex 会话管理</div>
        <label>Codex 主目录<input id="codex-home" value="${d(t.profile.codex_home)}" /></label>
        <label>项目<input id="project" value="${d(t.filter.project??"")}" /></label>
        <label>提供方<input id="provider" value="${d(t.filter.provider??"")}" /></label>
        <label>模型<input id="model" value="${d(t.filter.model??"")}" /></label>
        <label>来源<input id="source" value="${d(t.filter.source??"")}" /></label>
        <label>搜索<input id="search" value="${d(t.filter.search??"")}" /></label>
        <div class="segmented" role="group">
          ${h("all","全部")}
          ${h("active","活动")}
          ${h("archived","已归档")}
        </div>
        <button id="refresh" class="primary">刷新</button>
        <div class="edit-panel">
          <div class="edit-title">修改已选</div>
          <label>提供方<input id="edit-provider" placeholder="留空则不改" value="${d(t.selectedEdit.provider)}" /></label>
          <label>项目路径<input id="edit-project" placeholder="留空则不改" value="${d(t.selectedEdit.project)}" /></label>
          <div class="edit-actions">
            <button id="preview-selected-edit">预览</button>
            <button id="apply-selected-edit" class="primary">应用</button>
          </div>
        </div>
      </aside>
      <section class="workbench">
        <div class="toolbar">
          <div>${t.sessions.length} 个会话 · 已选 ${t.selectedIds.size} 个</div>
          <button id="probe" title="探测 app-server">探测</button>
          <button id="backup" title="创建备份">备份</button>
          <button id="archive" title="归档已选会话">归档</button>
          <button id="restore" title="恢复已选会话">恢复</button>
          <button id="delete" class="danger" title="将已选会话移入回收站">删除</button>
        </div>
        <div class="table" style="${A()}">
          ${q()}
          ${t.sessions.map(j).join("")}
        </div>
        <div class="status">${d(t.status)}</div>
      </section>
      <aside class="details">
        ${s?z(s):'<div class="empty">请选择一个会话</div>'}
      </aside>
    </main>
  `,W(),i&&R(i)}function h(e,i){return`<button data-archived="${e}" class="${t.filter.archived===e?"selected":""}">${i}</button>`}function q(){return`<div class="row header">${S.map((i,s)=>i.key==="select"?`
      <span class="header-cell select-header-cell">
        <input id="select-all" type="checkbox" aria-label="全选当前列表" ${k()?"checked":""} />
      </span>
    `:`
      <span class="header-cell">
        <span class="header-label">${d(i.label)}</span>
        ${i.resizable?`<span class="resize-handle" data-resize-column="${s}" role="separator" aria-label="调整${d(i.label)}列宽"></span>`:""}
      </span>
    `).join("")}</div>`}function j(e){const i=t.selectedIds.has(e.id);return`
    <button class="row ${t.activeId===e.id?"active":""}" data-open="${d(e.id)}">
      <input type="checkbox" data-select="${d(e.id)}" ${i?"checked":""} />
      <span>${d(e.title||e.first_user_message||e.id)}</span>
      <span>${d(e.project||"")}</span>
      <span>${d(e.provider||"")}</span>
      <span>${d(e.model||"")}</span>
      <span>${e.archived?"已归档":"活动"}</span>
      <span>${d(e.updated_at||"")}</span>
    </button>
  `}function z(e){return`
    <h2>${d(e.title||e.id)}</h2>
    <dl>
      <dt>ID</dt><dd>${d(e.id)}</dd>
      <dt>项目</dt><dd>${d(e.project||"")}</dd>
      <dt>提供方</dt><dd>${d(e.provider||"")}</dd>
      <dt>模型</dt><dd>${d(e.model||"")}</dd>
      <dt>来源</dt><dd>${d(e.source||"")}</dd>
      <dt>会话文件</dt><dd>${d(e.rollout_path||"")}</dd>
      <dt>会话索引</dt><dd>${e.in_session_index?"存在":"缺失"}</dd>
    </dl>
    <div class="detail-actions">
      <button data-single="archive">归档</button>
      <button data-single="restore">恢复</button>
      <button data-single="delete" class="danger">删除</button>
    </div>
  `}function W(){var i,s,l,n,o,a,p,b;u("codex-home",r=>t.profile.codex_home=r),u("project",r=>t.filter.project=v(r)),u("provider",r=>t.filter.provider=v(r)),u("model",r=>t.filter.model=v(r)),u("source",r=>t.filter.source=v(r)),u("search",r=>t.filter.search=v(r)),u("edit-provider",r=>t.selectedEdit.provider=r),u("edit-project",r=>t.selectedEdit.project=r),(i=document.querySelector("#refresh"))==null||i.addEventListener("click",$);const e=document.querySelector("#select-all");e&&(e.indeterminate=O()&&!k(),e.addEventListener("click",r=>r.stopPropagation()),e.addEventListener("change",()=>{e.checked?t.sessions.forEach(r=>t.selectedIds.add(r.id)):t.sessions.forEach(r=>t.selectedIds.delete(r.id)),c({preserveTableScroll:!0})})),(s=document.querySelector("#preview-selected-edit"))==null||s.addEventListener("click",()=>E(!1)),(l=document.querySelector("#apply-selected-edit"))==null||l.addEventListener("click",()=>E(!0)),(n=document.querySelector("#archive"))==null||n.addEventListener("click",()=>y("archive_sessions")),(o=document.querySelector("#restore"))==null||o.addEventListener("click",()=>y("restore_sessions")),(a=document.querySelector("#delete"))==null||a.addEventListener("click",()=>y("delete_sessions")),(p=document.querySelector("#backup"))==null||p.addEventListener("click",x),(b=document.querySelector("#probe"))==null||b.addEventListener("click",T),document.querySelectorAll("[data-archived]").forEach(r=>{r.addEventListener("click",()=>{t.filter.archived=r.dataset.archived,$()})}),document.querySelectorAll("[data-open]").forEach(r=>{r.addEventListener("click",()=>{t.activeId=r.dataset.open||"",c({preserveTableScroll:!0})})}),document.querySelectorAll("[data-select]").forEach(r=>{r.addEventListener("click",_=>{_.stopPropagation();const w=r.dataset.select||"";r.checked?t.selectedIds.add(w):t.selectedIds.delete(w),c({preserveTableScroll:!0})})}),document.querySelectorAll("[data-single]").forEach(r=>{r.addEventListener("click",()=>L(`${r.dataset.single}_sessions`,[t.activeId]))}),C()}function u(e,i){var s;(s=document.querySelector(`#${e}`))==null||s.addEventListener("change",l=>{i(l.target.value)})}async function $(){await m(async()=>{var e;t.sessions=await f("list_sessions",{profile:t.profile,filter:t.filter}),t.selectedIds.clear(),t.activeId=((e=t.sessions[0])==null?void 0:e.id)||"",t.status="已加载会话"})}async function y(e){await L(e,[...t.selectedIds])}async function L(e,i){if(i.length===0){t.status="请至少选择一个会话",c();return}await m(async()=>{const s=await f(e,{profile:t.profile,ids:i,apply:!0});t.status=JSON.stringify(s),await $()})}async function E(e){const i=[...t.selectedIds],s=t.selectedEdit.provider.trim(),l=t.selectedEdit.project.trim();if(i.length===0){t.status="请至少选择一个会话",c();return}if(!s&&!l){t.status="请填写提供方或项目路径",c({preserveTableScroll:!0});return}e&&!window.confirm(`将修改 ${i.length} 个已选会话，并在写入前创建备份。继续？`)||await m(async()=>{var o;const n=await f("edit_selected_sessions",{profile:t.profile,ids:i,edit:{provider:s||null,project:l||null},apply:e});e&&(t.sessions=await f("list_sessions",{profile:t.profile,filter:t.filter}),t.selectedIds.clear(),t.activeId=((o=t.sessions[0])==null?void 0:o.id)||""),t.status=P(n)})}async function x(){await m(async()=>{const e=await f("create_backup",{profile:t.profile,includeSessions:!1});t.status=JSON.stringify(e)})}async function T(){const e=window.prompt("App-server 端点","http://127.0.0.1:0");e&&await m(async()=>{const i=await f("app_server_probe",{profile:t.profile,endpoint:e});t.status=JSON.stringify(i)})}async function m(e){try{t.status="正在处理...",c(),await e()}catch(i){t.status=String(i)}finally{c()}}function P(e){const i=e.backup_dir?` · 备份 ${e.backup_dir}`:"";return`${e.action} · ${e.applied?"已应用":"预览"} · SQLite ${e.sqlite_rows} 行 · JSONL ${e.jsonl_files} 个${i}`}function A(){const e=t.columnWidths.map(s=>`${s}px`).join(" "),i=t.columnWidths.reduce((s,l)=>s+l,0);return`--session-grid: ${e}; --session-table-width: ${i}px;`}function N(){const e=document.querySelector(".table");if(!e)return;const i=t.columnWidths.map(l=>`${l}px`).join(" "),s=t.columnWidths.reduce((l,n)=>l+n,0);e.style.setProperty("--session-grid",i),e.style.setProperty("--session-table-width",`${s}px`)}function k(){return t.sessions.length>0&&t.sessions.every(e=>t.selectedIds.has(e.id))}function O(){return t.sessions.some(e=>t.selectedIds.has(e.id))}function M(){const e=document.querySelector(".table");return{left:(e==null?void 0:e.scrollLeft)??0,top:(e==null?void 0:e.scrollTop)??0}}function R(e){const i=document.querySelector(".table");i&&(i.scrollLeft=e.left,i.scrollTop=e.top)}function C(){document.querySelectorAll("[data-resize-column]").forEach(e=>{e.addEventListener("pointerdown",i=>{i.preventDefault();const s=Number(e.dataset.resizeColumn),l=S[s];if(!l)return;const n=i.clientX,o=t.columnWidths[s];document.body.classList.add("resizing-column");const a=b=>{const r=Math.max(l.minWidth,o+b.clientX-n);t.columnWidths[s]=Math.round(r),N()},p=()=>{document.body.classList.remove("resizing-column"),document.removeEventListener("pointermove",a),document.removeEventListener("pointerup",p),document.removeEventListener("pointercancel",p)};document.addEventListener("pointermove",a),document.addEventListener("pointerup",p),document.addEventListener("pointercancel",p)})})}function v(e){const i=e.trim();return i||void 0}function d(e){return e.replace(/[&<>"']/g,i=>({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#039;"})[i])}c();
