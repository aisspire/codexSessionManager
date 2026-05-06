(function(){const r=document.createElement("link").relList;if(r&&r.supports&&r.supports("modulepreload"))return;for(const s of document.querySelectorAll('link[rel="modulepreload"]'))o(s);new MutationObserver(s=>{for(const l of s)if(l.type==="childList")for(const a of l.addedNodes)a.tagName==="LINK"&&a.rel==="modulepreload"&&o(a)}).observe(document,{childList:!0,subtree:!0});function i(s){const l={};return s.integrity&&(l.integrity=s.integrity),s.referrerPolicy&&(l.referrerPolicy=s.referrerPolicy),s.crossOrigin==="use-credentials"?l.credentials="include":s.crossOrigin==="anonymous"?l.credentials="omit":l.credentials="same-origin",l}function o(s){if(s.ep)return;s.ep=!0;const l=i(s);fetch(s.href,l)}})();async function f(e,r={},i){return window.__TAURI_INTERNALS__.invoke(e,r,i)}const w=[{key:"select",label:"",width:42,minWidth:42,resizable:!1},{key:"session",label:"会话",width:280,minWidth:180,resizable:!0},{key:"project",label:"项目",width:220,minWidth:140,resizable:!0},{key:"provider",label:"提供方",width:120,minWidth:90,resizable:!0},{key:"model",label:"模型",width:150,minWidth:100,resizable:!0},{key:"state",label:"状态",width:110,minWidth:86,resizable:!0},{key:"updated",label:"更新时间",width:190,minWidth:140,resizable:!0}],t={profile:{codex_home:"~/.codex",path_maps:[]},filter:{archived:"all"},selectedEdit:{provider:"",project:""},sessions:[],selectedIds:new Set,activeId:"",status:"就绪",columnWidths:w.map(e=>e.width)},E=document.querySelector("#app");if(!E)throw new Error("missing app root");const _=E;function u(e={}){const r=e.preserveTableScroll?O():void 0,i=t.sessions.find(o=>o.id===t.activeId);_.innerHTML=`
    <main class="shell">
      <aside class="filters">
        <div class="brand">Codex 会话管理</div>
        <label>Codex 主目录<input id="codex-home" value="${n(t.profile.codex_home)}" /></label>
        <label>项目<input id="project" value="${n(t.filter.project??"")}" /></label>
        <label>提供方<input id="provider" value="${n(t.filter.provider??"")}" /></label>
        <label>模型<input id="model" value="${n(t.filter.model??"")}" /></label>
        <label>来源<input id="source" value="${n(t.filter.source??"")}" /></label>
        <label>搜索<input id="search" value="${n(t.filter.search??"")}" /></label>
        <div class="segmented" role="group">
          ${h("all","全部")}
          ${h("active","活动")}
          ${h("archived","已归档")}
        </div>
        <button id="refresh" class="primary">刷新</button>
        <div class="edit-panel">
          <div class="edit-title">修改已选</div>
          <label>提供方<input id="edit-provider" placeholder="留空则不改" value="${n(t.selectedEdit.provider)}" /></label>
          <label>项目路径<input id="edit-project" placeholder="留空则不改" value="${n(t.selectedEdit.project)}" /></label>
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
        <div class="table" style="${T()}">
          ${k()}
          ${t.sessions.map(I).join("")}
        </div>
        <div class="status">${n(t.status)}</div>
      </section>
      <aside class="details">
        ${i?q(i):'<div class="empty">请选择一个会话</div>'}
      </aside>
    </main>
  `,j(),r&&P(r)}function h(e,r){return`<button data-archived="${e}" class="${t.filter.archived===e?"selected":""}">${r}</button>`}function k(){return`<div class="row header">${w.map((r,i)=>`
      <span class="header-cell">
        <span class="header-label">${n(r.label)}</span>
        ${r.resizable?`<span class="resize-handle" data-resize-column="${i}" role="separator" aria-label="调整${n(r.label)}列宽"></span>`:""}
      </span>
    `).join("")}</div>`}function I(e){const r=t.selectedIds.has(e.id);return`
    <button class="row ${t.activeId===e.id?"active":""}" data-open="${n(e.id)}">
      <input type="checkbox" data-select="${n(e.id)}" ${r?"checked":""} />
      <span>${n(e.title||e.first_user_message||e.id)}</span>
      <span>${n(e.project||"")}</span>
      <span>${n(e.provider||"")}</span>
      <span>${n(e.model||"")}</span>
      <span>${e.archived?"已归档":"活动"}</span>
      <span>${n(e.updated_at||"")}</span>
    </button>
  `}function q(e){return`
    <h2>${n(e.title||e.id)}</h2>
    <dl>
      <dt>ID</dt><dd>${n(e.id)}</dd>
      <dt>项目</dt><dd>${n(e.project||"")}</dd>
      <dt>提供方</dt><dd>${n(e.provider||"")}</dd>
      <dt>模型</dt><dd>${n(e.model||"")}</dd>
      <dt>来源</dt><dd>${n(e.source||"")}</dd>
      <dt>会话文件</dt><dd>${n(e.rollout_path||"")}</dd>
      <dt>会话索引</dt><dd>${e.in_session_index?"存在":"缺失"}</dd>
    </dl>
    <div class="detail-actions">
      <button data-single="archive">归档</button>
      <button data-single="restore">恢复</button>
      <button data-single="delete" class="danger">删除</button>
    </div>
  `}function j(){var e,r,i,o,s,l,a,p;c("codex-home",d=>t.profile.codex_home=d),c("project",d=>t.filter.project=v(d)),c("provider",d=>t.filter.provider=v(d)),c("model",d=>t.filter.model=v(d)),c("source",d=>t.filter.source=v(d)),c("search",d=>t.filter.search=v(d)),c("edit-provider",d=>t.selectedEdit.provider=d),c("edit-project",d=>t.selectedEdit.project=d),(e=document.querySelector("#refresh"))==null||e.addEventListener("click",$),(r=document.querySelector("#preview-selected-edit"))==null||r.addEventListener("click",()=>g(!1)),(i=document.querySelector("#apply-selected-edit"))==null||i.addEventListener("click",()=>g(!0)),(o=document.querySelector("#archive"))==null||o.addEventListener("click",()=>y("archive_sessions")),(s=document.querySelector("#restore"))==null||s.addEventListener("click",()=>y("restore_sessions")),(l=document.querySelector("#delete"))==null||l.addEventListener("click",()=>y("delete_sessions")),(a=document.querySelector("#backup"))==null||a.addEventListener("click",z),(p=document.querySelector("#probe"))==null||p.addEventListener("click",W),document.querySelectorAll("[data-archived]").forEach(d=>{d.addEventListener("click",()=>{t.filter.archived=d.dataset.archived,$()})}),document.querySelectorAll("[data-open]").forEach(d=>{d.addEventListener("click",()=>{t.activeId=d.dataset.open||"",u({preserveTableScroll:!0})})}),document.querySelectorAll("[data-select]").forEach(d=>{d.addEventListener("click",b=>{b.stopPropagation();const S=d.dataset.select||"";d.checked?t.selectedIds.add(S):t.selectedIds.delete(S),u({preserveTableScroll:!0})})}),document.querySelectorAll("[data-single]").forEach(d=>{d.addEventListener("click",()=>L(`${d.dataset.single}_sessions`,[t.activeId]))}),A()}function c(e,r){var i;(i=document.querySelector(`#${e}`))==null||i.addEventListener("change",o=>{r(o.target.value)})}async function $(){await m(async()=>{var e;t.sessions=await f("list_sessions",{profile:t.profile,filter:t.filter}),t.selectedIds.clear(),t.activeId=((e=t.sessions[0])==null?void 0:e.id)||"",t.status="已加载会话"})}async function y(e){await L(e,[...t.selectedIds])}async function L(e,r){if(r.length===0){t.status="请至少选择一个会话",u();return}await m(async()=>{const i=await f(e,{profile:t.profile,ids:r,apply:!0});t.status=JSON.stringify(i),await $()})}async function g(e){const r=[...t.selectedIds],i=t.selectedEdit.provider.trim(),o=t.selectedEdit.project.trim();if(r.length===0){t.status="请至少选择一个会话",u();return}if(!i&&!o){t.status="请填写提供方或项目路径",u({preserveTableScroll:!0});return}e&&!window.confirm(`将修改 ${r.length} 个已选会话，并在写入前创建备份。继续？`)||await m(async()=>{var l;const s=await f("edit_selected_sessions",{profile:t.profile,ids:r,edit:{provider:i||null,project:o||null},apply:e});e&&(t.sessions=await f("list_sessions",{profile:t.profile,filter:t.filter}),t.selectedIds.clear(),t.activeId=((l=t.sessions[0])==null?void 0:l.id)||""),t.status=x(s)})}async function z(){await m(async()=>{const e=await f("create_backup",{profile:t.profile,includeSessions:!1});t.status=JSON.stringify(e)})}async function W(){const e=window.prompt("App-server 端点","http://127.0.0.1:0");e&&await m(async()=>{const r=await f("app_server_probe",{profile:t.profile,endpoint:e});t.status=JSON.stringify(r)})}async function m(e){try{t.status="正在处理...",u(),await e()}catch(r){t.status=String(r)}finally{u()}}function x(e){const r=e.backup_dir?` · 备份 ${e.backup_dir}`:"";return`${e.action} · ${e.applied?"已应用":"预览"} · SQLite ${e.sqlite_rows} 行 · JSONL ${e.jsonl_files} 个${r}`}function T(){const e=t.columnWidths.map(i=>`${i}px`).join(" "),r=t.columnWidths.reduce((i,o)=>i+o,0);return`--session-grid: ${e}; --session-table-width: ${r}px;`}function N(){const e=document.querySelector(".table");if(!e)return;const r=t.columnWidths.map(o=>`${o}px`).join(" "),i=t.columnWidths.reduce((o,s)=>o+s,0);e.style.setProperty("--session-grid",r),e.style.setProperty("--session-table-width",`${i}px`)}function O(){const e=document.querySelector(".table");return{left:(e==null?void 0:e.scrollLeft)??0,top:(e==null?void 0:e.scrollTop)??0}}function P(e){const r=document.querySelector(".table");r&&(r.scrollLeft=e.left,r.scrollTop=e.top)}function A(){document.querySelectorAll("[data-resize-column]").forEach(e=>{e.addEventListener("pointerdown",r=>{r.preventDefault();const i=Number(e.dataset.resizeColumn),o=w[i];if(!o)return;const s=r.clientX,l=t.columnWidths[i];document.body.classList.add("resizing-column");const a=d=>{const b=Math.max(o.minWidth,l+d.clientX-s);t.columnWidths[i]=Math.round(b),N()},p=()=>{document.body.classList.remove("resizing-column"),document.removeEventListener("pointermove",a),document.removeEventListener("pointerup",p),document.removeEventListener("pointercancel",p)};document.addEventListener("pointermove",a),document.addEventListener("pointerup",p),document.addEventListener("pointercancel",p)})})}function v(e){const r=e.trim();return r||void 0}function n(e){return e.replace(/[&<>"']/g,r=>({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#039;"})[r])}u();
