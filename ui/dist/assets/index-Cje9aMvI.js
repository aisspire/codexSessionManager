(function(){const r=document.createElement("link").relList;if(r&&r.supports&&r.supports("modulepreload"))return;for(const n of document.querySelectorAll('link[rel="modulepreload"]'))d(n);new MutationObserver(n=>{for(const s of n)if(s.type==="childList")for(const l of s.addedNodes)l.tagName==="LINK"&&l.rel==="modulepreload"&&d(l)}).observe(document,{childList:!0,subtree:!0});function i(n){const s={};return n.integrity&&(s.integrity=n.integrity),n.referrerPolicy&&(s.referrerPolicy=n.referrerPolicy),n.crossOrigin==="use-credentials"?s.credentials="include":n.crossOrigin==="anonymous"?s.credentials="omit":s.credentials="same-origin",s}function d(n){if(n.ep)return;n.ep=!0;const s=i(n);fetch(n.href,s)}})();async function f(e,r={},i){return window.__TAURI_INTERNALS__.invoke(e,r,i)}const g=[{key:"select",label:"",width:42,minWidth:42,resizable:!1},{key:"session",label:"会话",width:280,minWidth:180,resizable:!0},{key:"project",label:"项目",width:220,minWidth:140,resizable:!0},{key:"provider",label:"提供方",width:120,minWidth:90,resizable:!0},{key:"model",label:"模型",width:150,minWidth:100,resizable:!0},{key:"state",label:"状态",width:110,minWidth:86,resizable:!0},{key:"updated",label:"更新时间",width:190,minWidth:140,resizable:!0}],t={profile:{codex_home:"~/.codex",path_maps:[]},filter:{archived:"all"},providerMigration:{from:"codex-auto-review",to:"cm"},sessions:[],selectedIds:new Set,activeId:"",status:"就绪",columnWidths:g.map(e=>e.width)},E=document.querySelector("#app");if(!E)throw new Error("missing app root");const _=E;function p(e={}){const r=e.preserveTableScroll?P():void 0,i=t.sessions.find(d=>d.id===t.activeId);_.innerHTML=`
    <main class="shell">
      <aside class="filters">
        <div class="brand">Codex 会话管理</div>
        <label>Codex 主目录<input id="codex-home" value="${a(t.profile.codex_home)}" /></label>
        <label>项目<input id="project" value="${a(t.filter.project??"")}" /></label>
        <label>提供方<input id="provider" value="${a(t.filter.provider??"")}" /></label>
        <label>模型<input id="model" value="${a(t.filter.model??"")}" /></label>
        <label>来源<input id="source" value="${a(t.filter.source??"")}" /></label>
        <label>搜索<input id="search" value="${a(t.filter.search??"")}" /></label>
        <div class="segmented" role="group">
          ${h("all","全部")}
          ${h("active","活动")}
          ${h("archived","已归档")}
        </div>
        <button id="refresh" class="primary">刷新</button>
        <div class="migration-panel">
          <div class="migration-title">迁移提供方</div>
          <label>从<input id="provider-from" value="${a(t.providerMigration.from)}" /></label>
          <label>到<input id="provider-to" value="${a(t.providerMigration.to)}" /></label>
          <div class="migration-actions">
            <button id="preview-provider-migration">预览</button>
            <button id="apply-provider-migration" class="primary">应用</button>
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
        <div class="table" style="${j()}">
          ${k()}
          ${t.sessions.map(I).join("")}
        </div>
        <div class="status">${a(t.status)}</div>
      </section>
      <aside class="details">
        ${i?q(i):'<div class="empty">请选择一个会话</div>'}
      </aside>
    </main>
  `,z(),r&&N(r)}function h(e,r){return`<button data-archived="${e}" class="${t.filter.archived===e?"selected":""}">${r}</button>`}function k(){return`<div class="row header">${g.map((r,i)=>`
      <span class="header-cell">
        <span class="header-label">${a(r.label)}</span>
        ${r.resizable?`<span class="resize-handle" data-resize-column="${i}" role="separator" aria-label="调整${a(r.label)}列宽"></span>`:""}
      </span>
    `).join("")}</div>`}function I(e){const r=t.selectedIds.has(e.id);return`
    <button class="row ${t.activeId===e.id?"active":""}" data-open="${a(e.id)}">
      <input type="checkbox" data-select="${a(e.id)}" ${r?"checked":""} />
      <span>${a(e.title||e.first_user_message||e.id)}</span>
      <span>${a(e.project||"")}</span>
      <span>${a(e.provider||"")}</span>
      <span>${a(e.model||"")}</span>
      <span>${e.archived?"已归档":"活动"}</span>
      <span>${a(e.updated_at||"")}</span>
    </button>
  `}function q(e){return`
    <h2>${a(e.title||e.id)}</h2>
    <dl>
      <dt>ID</dt><dd>${a(e.id)}</dd>
      <dt>项目</dt><dd>${a(e.project||"")}</dd>
      <dt>提供方</dt><dd>${a(e.provider||"")}</dd>
      <dt>模型</dt><dd>${a(e.model||"")}</dd>
      <dt>来源</dt><dd>${a(e.source||"")}</dd>
      <dt>会话文件</dt><dd>${a(e.rollout_path||"")}</dd>
      <dt>会话索引</dt><dd>${e.in_session_index?"存在":"缺失"}</dd>
    </dl>
    <div class="detail-actions">
      <button data-single="archive">归档</button>
      <button data-single="restore">恢复</button>
      <button data-single="delete" class="danger">删除</button>
    </div>
  `}function z(){var e,r,i,d,n,s,l,u;c("codex-home",o=>t.profile.codex_home=o),c("project",o=>t.filter.project=m(o)),c("provider",o=>t.filter.provider=m(o)),c("model",o=>t.filter.model=m(o)),c("source",o=>t.filter.source=m(o)),c("search",o=>t.filter.search=m(o)),c("provider-from",o=>t.providerMigration.from=o),c("provider-to",o=>t.providerMigration.to=o),(e=document.querySelector("#refresh"))==null||e.addEventListener("click",$),(r=document.querySelector("#preview-provider-migration"))==null||r.addEventListener("click",()=>S(!1)),(i=document.querySelector("#apply-provider-migration"))==null||i.addEventListener("click",()=>S(!0)),(d=document.querySelector("#archive"))==null||d.addEventListener("click",()=>y("archive_sessions")),(n=document.querySelector("#restore"))==null||n.addEventListener("click",()=>y("restore_sessions")),(s=document.querySelector("#delete"))==null||s.addEventListener("click",()=>y("delete_sessions")),(l=document.querySelector("#backup"))==null||l.addEventListener("click",W),(u=document.querySelector("#probe"))==null||u.addEventListener("click",x),document.querySelectorAll("[data-archived]").forEach(o=>{o.addEventListener("click",()=>{t.filter.archived=o.dataset.archived,$()})}),document.querySelectorAll("[data-open]").forEach(o=>{o.addEventListener("click",()=>{t.activeId=o.dataset.open||"",p({preserveTableScroll:!0})})}),document.querySelectorAll("[data-select]").forEach(o=>{o.addEventListener("click",b=>{b.stopPropagation();const w=o.dataset.select||"";o.checked?t.selectedIds.add(w):t.selectedIds.delete(w),p({preserveTableScroll:!0})})}),document.querySelectorAll("[data-single]").forEach(o=>{o.addEventListener("click",()=>L(`${o.dataset.single}_sessions`,[t.activeId]))}),O()}function c(e,r){var i;(i=document.querySelector(`#${e}`))==null||i.addEventListener("change",d=>{r(d.target.value)})}async function $(){await v(async()=>{var e;t.sessions=await f("list_sessions",{profile:t.profile,filter:t.filter}),t.selectedIds.clear(),t.activeId=((e=t.sessions[0])==null?void 0:e.id)||"",t.status="已加载会话"})}async function y(e){await L(e,[...t.selectedIds])}async function L(e,r){if(r.length===0){t.status="请至少选择一个会话",p();return}await v(async()=>{const i=await f(e,{profile:t.profile,ids:r,apply:!0});t.status=JSON.stringify(i),await $()})}async function S(e){const r=t.providerMigration.from.trim(),i=t.providerMigration.to.trim();if(!r||!i){t.status="请填写来源和目标提供方",p();return}e&&!window.confirm(`将 ${r} 迁移为 ${i}，并在写入前创建备份。继续？`)||await v(async()=>{var n;const d=await f("migrate_provider",{profile:t.profile,from:r,to:i,apply:e});e&&(t.sessions=await f("list_sessions",{profile:t.profile,filter:t.filter}),t.selectedIds.clear(),t.activeId=((n=t.sessions[0])==null?void 0:n.id)||""),t.status=M(d)})}async function W(){await v(async()=>{const e=await f("create_backup",{profile:t.profile,includeSessions:!1});t.status=JSON.stringify(e)})}async function x(){const e=window.prompt("App-server 端点","http://127.0.0.1:0");e&&await v(async()=>{const r=await f("app_server_probe",{profile:t.profile,endpoint:e});t.status=JSON.stringify(r)})}async function v(e){try{t.status="正在处理...",p(),await e()}catch(r){t.status=String(r)}finally{p()}}function M(e){const r=e.backup_dir?` · 备份 ${e.backup_dir}`:"";return`${e.action} · ${e.applied?"已应用":"预览"} · SQLite ${e.sqlite_rows} 行 · JSONL ${e.jsonl_files} 个${r}`}function j(){const e=t.columnWidths.map(i=>`${i}px`).join(" "),r=t.columnWidths.reduce((i,d)=>i+d,0);return`--session-grid: ${e}; --session-table-width: ${r}px;`}function T(){const e=document.querySelector(".table");if(!e)return;const r=t.columnWidths.map(d=>`${d}px`).join(" "),i=t.columnWidths.reduce((d,n)=>d+n,0);e.style.setProperty("--session-grid",r),e.style.setProperty("--session-table-width",`${i}px`)}function P(){const e=document.querySelector(".table");return{left:(e==null?void 0:e.scrollLeft)??0,top:(e==null?void 0:e.scrollTop)??0}}function N(e){const r=document.querySelector(".table");r&&(r.scrollLeft=e.left,r.scrollTop=e.top)}function O(){document.querySelectorAll("[data-resize-column]").forEach(e=>{e.addEventListener("pointerdown",r=>{r.preventDefault();const i=Number(e.dataset.resizeColumn),d=g[i];if(!d)return;const n=r.clientX,s=t.columnWidths[i];document.body.classList.add("resizing-column");const l=o=>{const b=Math.max(d.minWidth,s+o.clientX-n);t.columnWidths[i]=Math.round(b),T()},u=()=>{document.body.classList.remove("resizing-column"),document.removeEventListener("pointermove",l),document.removeEventListener("pointerup",u),document.removeEventListener("pointercancel",u)};document.addEventListener("pointermove",l),document.addEventListener("pointerup",u),document.addEventListener("pointercancel",u)})})}function m(e){const r=e.trim();return r||void 0}function a(e){return e.replace(/[&<>"']/g,r=>({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#039;"})[r])}p();
