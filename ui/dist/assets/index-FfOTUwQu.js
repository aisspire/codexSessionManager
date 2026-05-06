(function(){const i=document.createElement("link").relList;if(i&&i.supports&&i.supports("modulepreload"))return;for(const d of document.querySelectorAll('link[rel="modulepreload"]'))l(d);new MutationObserver(d=>{for(const a of d)if(a.type==="childList")for(const o of a.addedNodes)o.tagName==="LINK"&&o.rel==="modulepreload"&&l(o)}).observe(document,{childList:!0,subtree:!0});function n(d){const a={};return d.integrity&&(a.integrity=d.integrity),d.referrerPolicy&&(a.referrerPolicy=d.referrerPolicy),d.crossOrigin==="use-credentials"?a.credentials="include":d.crossOrigin==="anonymous"?a.credentials="omit":a.credentials="same-origin",a}function l(d){if(d.ep)return;d.ep=!0;const a=n(d);fetch(d.href,a)}})();async function p(e,i={},n){return window.__TAURI_INTERNALS__.invoke(e,i,n)}const w=[{key:"select",label:"",width:42,minWidth:42,resizable:!1},{key:"session",label:"会话",width:280,minWidth:180,resizable:!0},{key:"project",label:"项目",width:220,minWidth:140,resizable:!0},{key:"provider",label:"提供方",width:120,minWidth:90,resizable:!0},{key:"model",label:"模型",width:150,minWidth:100,resizable:!0},{key:"state",label:"状态",width:110,minWidth:86,resizable:!0},{key:"updated",label:"更新时间",width:190,minWidth:140,resizable:!0}],t={profile:{codex_home:"~/.codex",path_maps:[]},filter:{archived:"all"},selectedEdit:{provider:"",project:"",titlePrefix:""},detailRename:{editing:!1,draft:"",pendingId:"",pendingTitle:""},sessions:[],selectedIds:new Set,activeId:"",status:"就绪",columnWidths:w.map(e=>e.width)},q=document.querySelector("#app");if(!q)throw new Error("missing app root");const W=q;function c(e={}){const i=e.preserveTableScroll?V():void 0,n=t.sessions.find(l=>l.id===t.activeId);W.innerHTML=`
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
          ${$("all","全部")}
          ${$("active","活动")}
          ${$("archived","已归档")}
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
          <button id="probe" title="探测 app-server">探测</button>
          <button id="backup" title="创建备份">备份</button>
          <button id="archive" title="归档已选会话">归档</button>
          <button id="restore" title="恢复已选会话">恢复</button>
          <button id="delete" class="danger" title="将已选会话移入回收站">删除</button>
        </div>
        <div class="table" style="${U()}">
          ${A()}
          ${t.sessions.map(N).join("")}
        </div>
        <div class="status">${s(t.status)}</div>
      </section>
      <aside class="details">
        ${n?O(n):'<div class="empty">请选择一个会话</div>'}
      </aside>
    </main>
  `,D(),i&&F(i)}function $(e,i){return`<button data-archived="${e}" class="${t.filter.archived===e?"selected":""}">${i}</button>`}function A(){return`<div class="row header">${w.map((i,n)=>i.key==="select"?`
      <span class="header-cell select-header-cell">
        <input id="select-all" type="checkbox" aria-label="全选当前列表" ${z()?"checked":""} />
      </span>
    `:`
      <span class="header-cell">
        <span class="header-label">${s(i.label)}</span>
        ${i.resizable?`<span class="resize-handle" data-resize-column="${n}" role="separator" aria-label="调整${s(i.label)}列宽"></span>`:""}
      </span>
    `).join("")}</div>`}function N(e){const i=t.selectedIds.has(e.id);return`
    <button class="row ${t.activeId===e.id?"active":""}" data-open="${s(e.id)}">
      <input type="checkbox" data-select="${s(e.id)}" ${i?"checked":""} />
      <span>${s(e.title||e.first_user_message||e.id)}</span>
      <span>${s(e.project||"")}</span>
      <span>${s(e.provider||"")}</span>
      <span>${s(e.model||"")}</span>
      <span>${e.archived?"已归档":"活动"}</span>
      <span>${s(e.updated_at||"")}</span>
    </button>
  `}function O(e){const i=g(e),n=b(e),l=R(e);return`
    <div class="detail-title-row">
      ${t.detailRename.editing&&t.detailRename.pendingId===e.id?`<input id="detail-title-input" class="detail-title-input" value="${s(t.detailRename.draft)}" />`:`<h2>${s(n||i)}</h2><button id="edit-detail-title" class="icon-button" title="重命名会话">✎</button>`}
    </div>
    <dl>
      <dt>ID</dt><dd>${s(e.id)}</dd>
      <dt>项目</dt><dd>${s(e.project||"")}</dd>
      <dt>提供方</dt><dd>${s(e.provider||"")}</dd>
      <dt>模型</dt><dd>${s(e.model||"")}</dd>
      <dt>来源</dt><dd>${s(e.source||"")}</dd>
      <dt>会话文件</dt><dd>${s(e.rollout_path||"")}</dd>
      <dt>会话索引</dt><dd>${e.in_session_index?"存在":"缺失"}</dd>
    </dl>
    <div class="detail-actions">
      <button id="save-detail-title" class="primary" ${l?"":"disabled"}>保存</button>
      <button data-single="archive">归档</button>
      <button data-single="restore">恢复</button>
      <button data-single="delete" class="danger">删除</button>
    </div>
  `}function D(){var n,l,d,a,o,f,h,y,L,I;u("codex-home",r=>t.profile.codex_home=r),u("project",r=>t.filter.project=m(r)),u("provider",r=>t.filter.provider=m(r)),u("model",r=>t.filter.model=m(r)),u("source",r=>t.filter.source=m(r)),u("search",r=>t.filter.search=m(r)),u("edit-title-prefix",r=>t.selectedEdit.titlePrefix=r),u("edit-provider",r=>t.selectedEdit.provider=r),u("edit-project",r=>t.selectedEdit.project=r),(n=document.querySelector("#refresh"))==null||n.addEventListener("click",E);const e=document.querySelector("#select-all");e&&(e.indeterminate=B()&&!z(),e.addEventListener("click",r=>r.stopPropagation()),e.addEventListener("change",()=>{e.checked?t.sessions.forEach(r=>t.selectedIds.add(r.id)):t.sessions.forEach(r=>t.selectedIds.delete(r.id)),c({preserveTableScroll:!0})})),(l=document.querySelector("#preview-selected-edit"))==null||l.addEventListener("click",()=>k(!1)),(d=document.querySelector("#apply-selected-edit"))==null||d.addEventListener("click",()=>k(!0)),(a=document.querySelector("#archive"))==null||a.addEventListener("click",()=>S("archive_sessions")),(o=document.querySelector("#restore"))==null||o.addEventListener("click",()=>S("restore_sessions")),(f=document.querySelector("#delete"))==null||f.addEventListener("click",()=>S("delete_sessions")),(h=document.querySelector("#backup"))==null||h.addEventListener("click",J),(y=document.querySelector("#probe"))==null||y.addEventListener("click",H),(L=document.querySelector("#edit-detail-title"))==null||L.addEventListener("click",M),(I=document.querySelector("#save-detail-title"))==null||I.addEventListener("click",C);const i=document.querySelector("#detail-title-input");i&&(i.focus(),i.select(),i.addEventListener("input",()=>{t.detailRename.draft=i.value}),i.addEventListener("keydown",r=>{r.key==="Enter"&&(T(),c({preserveTableScroll:!0}))}),i.addEventListener("blur",()=>{T(),window.setTimeout(()=>c({preserveTableScroll:!0}),0)})),document.querySelectorAll("[data-archived]").forEach(r=>{r.addEventListener("click",()=>{t.filter.archived=r.dataset.archived,E()})}),document.querySelectorAll("[data-open]").forEach(r=>{r.addEventListener("click",()=>{t.activeId=r.dataset.open||"",c({preserveTableScroll:!0})})}),document.querySelectorAll("[data-select]").forEach(r=>{r.addEventListener("click",P=>{P.stopPropagation();const _=r.dataset.select||"";r.checked?t.selectedIds.add(_):t.selectedIds.delete(_),c({preserveTableScroll:!0})})}),document.querySelectorAll("[data-single]").forEach(r=>{r.addEventListener("click",()=>x(`${r.dataset.single}_sessions`,[t.activeId]))}),K()}function u(e,i){var n;(n=document.querySelector(`#${e}`))==null||n.addEventListener("change",l=>{i(l.target.value)})}async function E(){await v(async()=>{var e;t.sessions=await p("list_sessions",{profile:t.profile,filter:t.filter}),t.selectedIds.clear(),t.activeId=((e=t.sessions[0])==null?void 0:e.id)||"",t.status="已加载会话"})}async function S(e){await x(e,[...t.selectedIds])}async function x(e,i){if(i.length===0){t.status="请至少选择一个会话",c();return}await v(async()=>{const n=await p(e,{profile:t.profile,ids:i,apply:!0});t.status=JSON.stringify(n),await E()})}async function k(e){const i=[...t.selectedIds],n=t.selectedEdit.provider.trim(),l=t.selectedEdit.project.trim(),d=t.selectedEdit.titlePrefix.trim();if(i.length===0){t.status="请至少选择一个会话",c();return}if(!n&&!l&&!d){t.status="请填写会话名前缀、提供方或项目路径",c({preserveTableScroll:!0});return}e&&!window.confirm(`将修改 ${i.length} 个已选会话，并在写入前创建备份。继续？`)||await v(async()=>{var o;const a=await p("edit_selected_sessions",{profile:t.profile,ids:i,edit:{provider:n||null,project:l||null,titlePrefix:d||null},apply:e});e&&(t.sessions=await p("list_sessions",{profile:t.profile,filter:t.filter}),t.selectedIds.clear(),t.activeId=((o=t.sessions[0])==null?void 0:o.id)||""),t.status=j(a)})}function M(){const e=t.sessions.find(i=>i.id===t.activeId);e&&(t.detailRename={editing:!0,draft:b(e)||g(e),pendingId:e.id,pendingTitle:b(e)},c({preserveTableScroll:!0}))}function T(){const e=t.sessions.find(n=>n.id===t.activeId);if(!e||t.detailRename.pendingId!==e.id)return;const i=t.detailRename.draft.trim();t.detailRename.editing=!1,t.detailRename.pendingTitle=i||g(e)}async function C(){const e=t.sessions.find(n=>n.id===t.activeId);if(!e||!R(e))return;const i=b(e);await v(async()=>{var d;const n=await p("edit_selected_sessions",{profile:t.profile,ids:[e.id],edit:{title:i},apply:!0}),l=e.id;t.sessions=await p("list_sessions",{profile:t.profile,filter:t.filter}),t.activeId=t.sessions.some(a=>a.id===l)?l:((d=t.sessions[0])==null?void 0:d.id)||"",t.detailRename={editing:!1,draft:"",pendingId:"",pendingTitle:""},t.status=j(n)})}function g(e){return e.title||e.first_user_message||e.id}function b(e){return t.detailRename.pendingId===e.id?t.detailRename.pendingTitle.trim():""}function R(e){const i=b(e);return i.length>0&&i!==g(e)}async function J(){await v(async()=>{const e=await p("create_backup",{profile:t.profile,includeSessions:!1});t.status=JSON.stringify(e)})}async function H(){const e=window.prompt("App-server 端点","http://127.0.0.1:0");e&&await v(async()=>{const i=await p("app_server_probe",{profile:t.profile,endpoint:e});t.status=JSON.stringify(i)})}async function v(e){try{t.status="正在处理...",c(),await e()}catch(i){t.status=String(i)}finally{c()}}function j(e){const i=e.backup_dir?` · 备份 ${e.backup_dir}`:"";return`${e.action} · ${e.applied?"已应用":"预览"} · SQLite ${e.sqlite_rows} 行 · JSONL ${e.jsonl_files} 个${i}`}function U(){const e=t.columnWidths.map(n=>`${n}px`).join(" "),i=t.columnWidths.reduce((n,l)=>n+l,0);return`--session-grid: ${e}; --session-table-width: ${i}px;`}function X(){const e=document.querySelector(".table");if(!e)return;const i=t.columnWidths.map(l=>`${l}px`).join(" "),n=t.columnWidths.reduce((l,d)=>l+d,0);e.style.setProperty("--session-grid",i),e.style.setProperty("--session-table-width",`${n}px`)}function z(){return t.sessions.length>0&&t.sessions.every(e=>t.selectedIds.has(e.id))}function B(){return t.sessions.some(e=>t.selectedIds.has(e.id))}function V(){const e=document.querySelector(".table");return{left:(e==null?void 0:e.scrollLeft)??0,top:(e==null?void 0:e.scrollTop)??0}}function F(e){const i=document.querySelector(".table");i&&(i.scrollLeft=e.left,i.scrollTop=e.top)}function K(){document.querySelectorAll("[data-resize-column]").forEach(e=>{e.addEventListener("pointerdown",i=>{i.preventDefault();const n=Number(e.dataset.resizeColumn),l=w[n];if(!l)return;const d=i.clientX,a=t.columnWidths[n];document.body.classList.add("resizing-column");const o=h=>{const y=Math.max(l.minWidth,a+h.clientX-d);t.columnWidths[n]=Math.round(y),X()},f=()=>{document.body.classList.remove("resizing-column"),document.removeEventListener("pointermove",o),document.removeEventListener("pointerup",f),document.removeEventListener("pointercancel",f)};document.addEventListener("pointermove",o),document.addEventListener("pointerup",f),document.addEventListener("pointercancel",f)})})}function m(e){const i=e.trim();return i||void 0}function s(e){return e.replace(/[&<>"']/g,i=>({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#039;"})[i])}c();
