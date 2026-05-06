(function(){const i=document.createElement("link").relList;if(i&&i.supports&&i.supports("modulepreload"))return;for(const n of document.querySelectorAll('link[rel="modulepreload"]'))l(n);new MutationObserver(n=>{for(const a of n)if(a.type==="childList")for(const o of a.addedNodes)o.tagName==="LINK"&&o.rel==="modulepreload"&&l(o)}).observe(document,{childList:!0,subtree:!0});function d(n){const a={};return n.integrity&&(a.integrity=n.integrity),n.referrerPolicy&&(a.referrerPolicy=n.referrerPolicy),n.crossOrigin==="use-credentials"?a.credentials="include":n.crossOrigin==="anonymous"?a.credentials="omit":a.credentials="same-origin",a}function l(n){if(n.ep)return;n.ep=!0;const a=d(n);fetch(n.href,a)}})();async function f(e,i={},d){return window.__TAURI_INTERNALS__.invoke(e,i,d)}const w=[{key:"select",label:"",width:42,minWidth:42,resizable:!1},{key:"session",label:"会话",width:280,minWidth:180,resizable:!0},{key:"project",label:"项目",width:220,minWidth:140,resizable:!0},{key:"provider",label:"提供方",width:120,minWidth:90,resizable:!0},{key:"model",label:"模型",width:150,minWidth:100,resizable:!0},{key:"state",label:"状态",width:110,minWidth:86,resizable:!0},{key:"updated",label:"更新时间",width:190,minWidth:140,resizable:!0}],t={profile:{codex_home:"~/.codex",path_maps:[]},filter:{archived:"all"},selectedEdit:{provider:"",project:"",titlePrefix:""},detailEdit:{editingField:"",draft:"",pendingId:"",pendingTitle:"",pendingProject:"",pendingProvider:""},sessions:[],selectedIds:new Set,activeId:"",status:"就绪",columnWidths:w.map(e=>e.width)},P=document.querySelector("#app");if(!P)throw new Error("missing app root");const N=P;function c(e={}){const i=e.preserveTableScroll?Q():void 0,d=t.sessions.find(l=>l.id===t.activeId);N.innerHTML=`
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
          ${g("all","全部")}
          ${g("active","活动")}
          ${g("archived","已归档")}
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
        <div class="table" style="${X()}">
          ${O()}
          ${t.sessions.map(D).join("")}
        </div>
        <div class="status">${s(t.status)}</div>
      </section>
      <aside class="details">
        ${d?F(d):'<div class="empty">请选择一个会话</div>'}
      </aside>
    </main>
  `,R(),i&&G(i)}function g(e,i){return`<button data-archived="${e}" class="${t.filter.archived===e?"selected":""}">${i}</button>`}function O(){return`<div class="row header">${w.map((i,d)=>i.key==="select"?`
      <span class="header-cell select-header-cell">
        <input id="select-all" type="checkbox" aria-label="全选当前列表" ${W()?"checked":""} />
      </span>
    `:`
      <span class="header-cell">
        <span class="header-label">${s(i.label)}</span>
        ${i.resizable?`<span class="resize-handle" data-resize-column="${d}" role="separator" aria-label="调整${s(i.label)}列宽"></span>`:""}
      </span>
    `).join("")}</div>`}function D(e){const i=t.selectedIds.has(e.id);return`
    <button class="row ${t.activeId===e.id?"active":""}" data-open="${s(e.id)}">
      <input type="checkbox" data-select="${s(e.id)}" ${i?"checked":""} />
      <span>${s(e.title||e.first_user_message||e.id)}</span>
      <span>${s(e.project||"")}</span>
      <span>${s(e.provider||"")}</span>
      <span>${s(e.model||"")}</span>
      <span>${e.archived?"已归档":"活动"}</span>
      <span>${s(e.updated_at||"")}</span>
    </button>
  `}function F(e){const i=T(e),d=v(e,"title"),l=x(e);return`
    <div class="detail-title-row">
      ${t.detailEdit.editingField==="title"&&t.detailEdit.pendingId===e.id?`<input id="detail-edit-input" class="detail-title-input" value="${s(t.detailEdit.draft)}" />`:`<h2>${s(d||i)}</h2><button data-detail-edit="title" class="icon-button" title="重命名会话">✎</button>`}
    </div>
    <dl>
      <dt>ID</dt><dd>${s(e.id)}</dd>
      ${k(e,"项目","project")}
      ${k(e,"提供方","provider")}
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
  `}function R(){var d,l,n,a,o,u,h,y,I;p("codex-home",r=>t.profile.codex_home=r),p("project",r=>t.filter.project=b(r)),p("provider",r=>t.filter.provider=b(r)),p("model",r=>t.filter.model=b(r)),p("source",r=>t.filter.source=b(r)),p("search",r=>t.filter.search=b(r)),p("edit-title-prefix",r=>t.selectedEdit.titlePrefix=r),p("edit-provider",r=>t.selectedEdit.provider=r),p("edit-project",r=>t.selectedEdit.project=r),(d=document.querySelector("#refresh"))==null||d.addEventListener("click",S);const e=document.querySelector("#select-all");e&&(e.indeterminate=K()&&!W(),e.addEventListener("click",r=>r.stopPropagation()),e.addEventListener("change",()=>{e.checked?t.sessions.forEach(r=>t.selectedIds.add(r.id)):t.sessions.forEach(r=>t.selectedIds.delete(r.id)),c({preserveTableScroll:!0})})),(l=document.querySelector("#preview-selected-edit"))==null||l.addEventListener("click",()=>_(!1)),(n=document.querySelector("#apply-selected-edit"))==null||n.addEventListener("click",()=>_(!0)),(a=document.querySelector("#archive"))==null||a.addEventListener("click",()=>$("archive_sessions")),(o=document.querySelector("#restore"))==null||o.addEventListener("click",()=>$("restore_sessions")),(u=document.querySelector("#delete"))==null||u.addEventListener("click",()=>$("delete_sessions")),(h=document.querySelector("#backup"))==null||h.addEventListener("click",H),(y=document.querySelector("#probe"))==null||y.addEventListener("click",U),document.querySelectorAll("[data-detail-edit]").forEach(r=>{r.addEventListener("click",()=>C(r.dataset.detailEdit))}),(I=document.querySelector("#save-detail-title"))==null||I.addEventListener("click",M);const i=document.querySelector("#detail-edit-input");i&&(i.focus(),i.select(),i.addEventListener("input",()=>{t.detailEdit.draft=i.value}),i.addEventListener("keydown",r=>{r.key==="Enter"&&(j(),c({preserveTableScroll:!0}))}),i.addEventListener("blur",()=>{j(),window.setTimeout(()=>c({preserveTableScroll:!0}),0)})),document.querySelectorAll("[data-archived]").forEach(r=>{r.addEventListener("click",()=>{t.filter.archived=r.dataset.archived,S()})}),document.querySelectorAll("[data-open]").forEach(r=>{r.addEventListener("click",()=>{t.activeId=r.dataset.open||"",c({preserveTableScroll:!0})})}),document.querySelectorAll("[data-select]").forEach(r=>{r.addEventListener("click",A=>{A.stopPropagation();const L=r.dataset.select||"";r.checked?t.selectedIds.add(L):t.selectedIds.delete(L),c({preserveTableScroll:!0})})}),document.querySelectorAll("[data-single]").forEach(r=>{r.addEventListener("click",()=>q(`${r.dataset.single}_sessions`,[t.activeId]))}),Y()}function p(e,i){var d;(d=document.querySelector(`#${e}`))==null||d.addEventListener("change",l=>{i(l.target.value)})}async function S(){await m(async()=>{var e;t.sessions=await f("list_sessions",{profile:t.profile,filter:t.filter}),t.selectedIds.clear(),t.activeId=((e=t.sessions[0])==null?void 0:e.id)||"",t.status="已加载会话"})}async function $(e){await q(e,[...t.selectedIds])}async function q(e,i){if(i.length===0){t.status="请至少选择一个会话",c();return}await m(async()=>{const d=await f(e,{profile:t.profile,ids:i,apply:!0});t.status=JSON.stringify(d),await S()})}async function _(e){const i=[...t.selectedIds],d=t.selectedEdit.provider.trim(),l=t.selectedEdit.project.trim(),n=t.selectedEdit.titlePrefix.trim();if(i.length===0){t.status="请至少选择一个会话",c();return}if(!d&&!l&&!n){t.status="请填写会话名前缀、提供方或项目路径",c({preserveTableScroll:!0});return}e&&!window.confirm(`将修改 ${i.length} 个已选会话，并在写入前创建备份。继续？`)||await m(async()=>{var o;const a=await f("edit_selected_sessions",{profile:t.profile,ids:i,edit:{provider:d||null,project:l||null,titlePrefix:n||null},apply:e});e&&(t.sessions=await f("list_sessions",{profile:t.profile,filter:t.filter}),t.selectedIds.clear(),t.activeId=((o=t.sessions[0])==null?void 0:o.id)||""),t.status=z(a)})}function k(e,i,d){const l=t.detailEdit.editingField===d&&t.detailEdit.pendingId===e.id,n=l?t.detailEdit.draft:V(e,d);return`
    <dt>${s(i)}</dt>
    <dd class="detail-editable-value">
      ${l?`<input id="detail-edit-input" class="detail-inline-input" value="${s(n)}" />`:`<span>${s(n)}</span><button data-detail-edit="${d}" class="icon-button" title="修改${s(i)}">✎</button>`}
    </dd>
  `}function C(e){const i=t.sessions.find(d=>d.id===t.activeId);i&&(t.detailEdit={...t.detailEdit,editingField:e,draft:v(i,e)||E(i,e),pendingId:i.id},c({preserveTableScroll:!0}))}function j(){const e=t.sessions.find(l=>l.id===t.activeId),i=t.detailEdit.editingField;if(!e||!i||t.detailEdit.pendingId!==e.id)return;const d=t.detailEdit.draft.trim()||E(e,i);t.detailEdit.editingField="",J(i,d)}async function M(){const e=t.sessions.find(n=>n.id===t.activeId);if(!e||!x(e))return;const i=v(e,"title"),d=v(e,"project"),l=v(e,"provider");await m(async()=>{var o;const n=await f("edit_selected_sessions",{profile:t.profile,ids:[e.id],edit:{title:i||null,project:d||null,provider:l||null},apply:!0}),a=e.id;t.sessions=await f("list_sessions",{profile:t.profile,filter:t.filter}),t.activeId=t.sessions.some(u=>u.id===a)?a:((o=t.sessions[0])==null?void 0:o.id)||"",t.detailEdit={editingField:"",draft:"",pendingId:"",pendingTitle:"",pendingProject:"",pendingProvider:""},t.status=z(n)})}function T(e){return e.title||e.first_user_message||e.id}function E(e,i){return i==="title"?T(e):i==="project"?e.project||"":e.provider||""}function V(e,i){return v(e,i)||E(e,i)}function v(e,i){return t.detailEdit.pendingId!==e.id?"":i==="title"?t.detailEdit.pendingTitle.trim():i==="project"?t.detailEdit.pendingProject.trim():t.detailEdit.pendingProvider.trim()}function J(e,i){e==="title"?t.detailEdit.pendingTitle=i:e==="project"?t.detailEdit.pendingProject=i:t.detailEdit.pendingProvider=i}function x(e){return["title","project","provider"].some(i=>{const d=v(e,i);return d.length>0&&d!==E(e,i)})}async function H(){await m(async()=>{const e=await f("create_backup",{profile:t.profile,includeSessions:!1});t.status=JSON.stringify(e)})}async function U(){const e=window.prompt("App-server 端点","http://127.0.0.1:0");e&&await m(async()=>{const i=await f("app_server_probe",{profile:t.profile,endpoint:e});t.status=JSON.stringify(i)})}async function m(e){try{t.status="正在处理...",c(),await e()}catch(i){t.status=String(i)}finally{c()}}function z(e){const i=e.backup_dir?` · 备份 ${e.backup_dir}`:"";return`${e.action} · ${e.applied?"已应用":"预览"} · SQLite ${e.sqlite_rows} 行 · JSONL ${e.jsonl_files} 个${i}`}function X(){const e=t.columnWidths.map(d=>`${d}px`).join(" "),i=t.columnWidths.reduce((d,l)=>d+l,0);return`--session-grid: ${e}; --session-table-width: ${i}px;`}function B(){const e=document.querySelector(".table");if(!e)return;const i=t.columnWidths.map(l=>`${l}px`).join(" "),d=t.columnWidths.reduce((l,n)=>l+n,0);e.style.setProperty("--session-grid",i),e.style.setProperty("--session-table-width",`${d}px`)}function W(){return t.sessions.length>0&&t.sessions.every(e=>t.selectedIds.has(e.id))}function K(){return t.sessions.some(e=>t.selectedIds.has(e.id))}function Q(){const e=document.querySelector(".table");return{left:(e==null?void 0:e.scrollLeft)??0,top:(e==null?void 0:e.scrollTop)??0}}function G(e){const i=document.querySelector(".table");i&&(i.scrollLeft=e.left,i.scrollTop=e.top)}function Y(){document.querySelectorAll("[data-resize-column]").forEach(e=>{e.addEventListener("pointerdown",i=>{i.preventDefault();const d=Number(e.dataset.resizeColumn),l=w[d];if(!l)return;const n=i.clientX,a=t.columnWidths[d];document.body.classList.add("resizing-column");const o=h=>{const y=Math.max(l.minWidth,a+h.clientX-n);t.columnWidths[d]=Math.round(y),B()},u=()=>{document.body.classList.remove("resizing-column"),document.removeEventListener("pointermove",o),document.removeEventListener("pointerup",u),document.removeEventListener("pointercancel",u)};document.addEventListener("pointermove",o),document.addEventListener("pointerup",u),document.addEventListener("pointercancel",u)})})}function b(e){const i=e.trim();return i||void 0}function s(e){return e.replace(/[&<>"']/g,i=>({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#039;"})[i])}c();
