(window.webpackJsonp=window.webpackJsonp||[]).push([[2],{dPvw:function(e,t,s){"use strict";function i(){return i=Object.assign||function(e){for(var t=1;t<arguments.length;t++){var s=arguments[t];for(var i in s)Object.prototype.hasOwnProperty.call(s,i)&&(e[i]=s[i])}return e},i.apply(this,arguments)}s.r(t);var n=s("4Iz4"),o=s("lBHI"),a=s("DrMS"),l=s("5OaP"),r=s("jMw0"),p=s("wCQ/"),c=s.n(p),h=s("oeWf"),d=s("vI8o");let u,$,b,m,f,g,w,O,y,C,v,j,I,x,k,S,N,U,P,R,L,M,A,G,z,E,T,_=e=>e;const D=/^(?:[A-Za-z0-9+/]{4}){10}(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)+$/,Z=/^[A-Za-z0-9\-\_]{40,50}\.[A-Za-z0-9\_\-]{40,50}$/,F=Object(n.a)(u||(u=_`
<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="currentColor" class="bi bi-chevron-down" viewBox="0 0 16 16">
  <path fill-rule="evenodd" d="M1.646 4.646a.5.5 0 0 1 .708 0L8 10.293l5.646-5.647a.5.5 0 0 1 .708.708l-6 6a.5.5 0 0 1-.708 0l-6-6a.5.5 0 0 1 0-.708z"/>
</svg>
`)),K=Object(n.a)($||($=_`
<svg width="12" height="12" fill="currentColor" viewBox="0 0 16 16">
  <path fill-rule="evenodd" d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z"/>
</svg>
`));class B extends l.a{constructor(){super(),this.children=[],this.state={groups:{},children:{},shownChildrenCount:50}}getNode(){if(this.props.path.length>1){let e=this.props.path;return 0===e.indexOf("Public/Users")&&(e=e.replace("/Users","")),e=e.split("/"),e.slice(1).reduce(((e,t)=>t&&e.get(decodeURIComponent(t))||e),this.props.gun)}return this.props.gun}shouldComponentUpdate(){return!0}componentDidMount(){var e=this;if(this.isMine=0===this.props.path.indexOf(`Public/Users/~${r.a.getPubKey()}`),this.isGroup=0===this.props.path.indexOf("Group/"),this.isPublicRoot="Public"===this.props.path,this.isUserList="Public/Users"===this.props.path,this.isGroupRoot="Group"===this.props.path,this.isLocal=0===this.props.path.indexOf("Local"),this.children={},this.props.children&&"object"==typeof this.props.children&&(this.children=i(this.children,this.props.children)),this.isPublicRoot&&(this.children=i(this.children,{"#":{value:{_:1},displayName:"ContentAddressed"},Users:{value:{_:1}}})),this.isUserList){const e={};e[`~${r.a.getPubKey()}`]={value:{_:1}},this.children=i(this.children,e)}if(this.isGroupRoot){const e={};o.a.local.get("groups").map(this.sub(((t,s)=>{t?e[s]=!0:delete e[s],this.setState({groups:e})})))}this.setState({children:this.children,shownChildrenCount:50});const t=this.sub((async function(t,s,n,o,a){if("_"===s)return;if(e.isPublicRoot&&0===s.indexOf("~"))return;if(e.isUserList&&!s.substr(1).match(Z))return;let l;if("string"==typeof t&&0===t.indexOf("SEA{"))try{const s=r.a.getKey();let i=await c.a.SEA.decrypt(t,s);void 0===i&&(e.mySecret||(e.mySecret=await c.a.SEA.secret(s.epub,s),i=await c.a.SEA.decrypt(t,e.mySecret))),void 0!==i?(t=i,l="Decrypted"):l="Encrypted"}catch(o){}e.children[s]=i(e.children[s]||{},{value:t,encryption:l,from:a}),e.setState({children:e.children})}));if(!this.isGroupRoot)if(this.isGroup){const e=this.props.path.split("/").slice(2).join("/");this.props.gun.map(e,t)}else this.getNode().map(t)}onChildObjectClick(e,t){e.preventDefault(),this.children[t].open=!this.children[t].open,this.setState({children:this.children})}onShowMoreClick(e,t){e.preventDefault(),this.children[t].showMore=!this.children[t].showMore,this.setState({children:this.children})}renderChildObject(e,t){const s=`${this.props.path}/${encodeURIComponent(e)}`,i=e.substr(1);return Object(n.a)(b||(b=_`
      <div class="explorer-row" style="padding-left: ${0}em">
        <span onClick=${0}>${0}</span>
        <a href="/explorer/${0}">
            <b>
                ${0}
            </b>
        </a>
        ${0}
      </div>
      ${0}
    `),this.props.indent,(t=>this.onChildObjectClick(t,e)),this.state.children[e].open?F:K,encodeURIComponent(s),"string"==typeof e&&i.match(Z)?Object(n.a)(m||(m=_`<${0} key=${0} pub=${0} placeholder="user"/>`),h.a,e,i):t||e,r.a.getPubKey()===i?Object(n.a)(f||(f=_`<small class="mar-left5">(you)</small>`)):"",this.state.children[e].open?Object(n.a)(g||(g=_`<${0} gun=${0} indent=${0} key=${0} path=${0} isGroup=${0}/>`),B,this.props.gun,this.props.indent+1,s,s,this.props.isGroup):"")}renderChildValue(e,t){let s;const i=this.children[e].encryption,o=this.children[e].from,a="Decrypted"===i,l=(e,t,s)=>Object(n.a)(w||(w=_`<a class=${0} href=${0}>${0}</a>`),void 0===s?"mar-left5":s,e,t),p=Object(n.a)(O||(O=_`
      ${0}
      ${0}
    `),"string"==typeof e&&e.match(D)?l(`/post/${encodeURIComponent(e)}`,"#"):"","string"==typeof e&&e.match(Z)?l(`/explorer/Public%2F~${encodeURIComponent(encodeURIComponent(e))}`,Object(n.a)(y||(y=_`<${0} key=${0} pub=${0} placeholder="user"/>`),h.a,e,e)):"");if(i)s=a?JSON.stringify(t):Object(n.a)(C||(C=_`<i>Encrypted value</i>`));else{const i=r.a.getPubKey(),a=(this.isMine||this.isLocal)&&`${this.props.path}/${encodeURIComponent(e)}`.replace(`Public/Users/~${i}/`,"").replace("Local/","");if("string"==typeof t&&0===t.indexOf("data:image"))s=this.isMine?Object(n.a)(v||(v=_`<iris-img user=${0} path=${0}/>`),i,a):Object(n.a)(j||(j=_`<img src=${0}/>`),t);else{let r,p=JSON.stringify(t);p.length>100&&(r=!0,this.state.children[e].showMore||(p=p.slice(0,100)));const c=Object(n.a)(I||(I=_`
          ${0}
          ${0}
          ${0}
        `),"string"==typeof t&&t.match(D)?l(`/post/${encodeURIComponent(t)}`,"#"):"","epub"!==e&&"string"==typeof t&&t.match(Z)?l(`/explorer/Public%2F~${encodeURIComponent(encodeURIComponent(t))}`,Object(n.a)(x||(x=_`<${0} key=${0} pub=${0} placeholder="user"/>`),h.a,t,t)):"","string"==typeof o?Object(n.a)(k||(k=_`<small> from ${0}</small>`),l(`/explorer/Public%2F~${encodeURIComponent(encodeURIComponent(o))}`,Object(n.a)(S||(S=_`<${0} key=${0} pub=${0} placeholder="user"/>`),h.a,o,o),"")):"");s=this.isMine||this.isLocal?Object(n.a)(N||(N=_`
          <${0} gun=${0} placeholder="empty" key=${0} user=${0} path=${0} editable=${0} json=${0}/>
          ${0}
        `),d.a,this.props.gun,a,this.isLocal?null:i,a,!0,!0,c):Object(n.a)(U||(U=_`
          <span class=${0}>
            ${0}
            ${0}
            ${0}
          </span>
        `),"string"==typeof t?"":"iris-non-string",p,r?Object(n.a)(P||(P=_`
              <a onClick=${0} href="">${0}</a>
            `),(t=>this.onShowMoreClick(t,e)),this.state.children[e].showMore?"less":"more"):"",c)}}return Object(n.a)(R||(R=_`
      <div class="explorer-row" style="padding-left: ${0}em">
        <b class="val">${0} ${0}</b>:
        ${0} ${0}
      </div>
    `),this.props.indent,e,p,i?Object(n.a)(L||(L=_`
          <span class="tooltip"><span class="tooltiptext">${0} value</span>
            ${0}
          </span>
        `),i,a?"🔓":""):"",s)}onExpandClicked(){const e=!this.state.expandAll;Object.keys(this.children).forEach((t=>{this.children[t].open=e})),this.setState({expandAll:e,children:this.children})}onNewItemSubmit(e){if(e.preventDefault(),this.state.newItemName){let e=this.state.newItemName.trim();"object"===this.state.newItemType?this.getNode().get(e).put({a:null}):this.getNode().get(e).put(""),this.setState({newItemType:!1,newItemName:""})}}onNewItemNameInput(e){this.setState({newItemName:e.target.value.trimStart().replace("  "," ")})}showNewItemClicked(e){this.setState({newItemType:e}),setTimeout((()=>document.querySelector("#newItemNameInput").focus()),0)}render(){const e=Object.keys(this.state.children).sort(),t=e.indexOf(`~${r.a.getPubKey()}`);if(t>0){const s=e.splice(t,1);e.unshift(s[0])}const s=e.length>this.state.shownChildrenCount;return Object(n.a)(M||(M=_`
      ${0}
      
      ${0}
      
      ${0}
    `),0===this.props.indent?Object(n.a)(A||(A=_`
        <div class="explorer-row" style="padding-left: ${0}em">
          ${0}
          ${0}
        </div>
      `),this.props.indent,this.props.showTools?Object(n.a)(G||(G=_`
            <p class="explorer-tools">
              <a onClick=${0}>${0}</a>
              <a onClick=${0}>New object</a>
              <a onClick=${0}>New value</a>
              ${0} items
            </p>
          `),(()=>this.onExpandClicked()),this.state.expandAll?"Close all":"Expand all",(()=>this.showNewItemClicked("object")),(()=>this.showNewItemClicked("value")),e.length):"",this.state.newItemType?Object(n.a)(z||(z=_`
            <p>
              <form onSubmit=${0}>
                <input id="newItemNameInput" type="text" onInput=${0} value=${0} placeholder="New ${0} name"/>
                <button type="submit">Create</button>
                <button onClick=${0}>Cancel</button>
              </form>
            </p>
          `),(e=>this.onNewItemSubmit(e)),(e=>this.onNewItemNameInput(e)),this.state.newItemName,this.state.newItemType,(()=>this.setState({newItemType:!1}))):""):"",this.isGroupRoot?Object.keys(this.state.groups).map((e=>Object(n.a)(E||(E=_`
          <div class="explorer-row" style="padding-left: 1em">
              ${0}
              <a href="/explorer/Group%2F${0}"><b>${0}</b></a>
          </div>
      `),K,encodeURIComponent(encodeURIComponent(e)),e))):(e=>e.map((e=>{const t=this.state.children[e].value;return"object"==typeof t&&t&&t._?this.renderChildObject(e,this.state.children[e].displayName):this.renderChildValue(e,t)})))(e.slice(0,this.state.shownChildrenCount)),s?Object(n.a)(T||(T=_`
        <a style="padding-left: ${0}em" href="" onClick=${0}>More (${0})</a>
      `),this.props.indent+1,(e=>{e.preventDefault(),this.setState({shownChildrenCount:this.state.shownChildrenCount+50})}),e.length-this.state.shownChildrenCount):"")}}var J=B,V=s("3rgF");let q,H,Q,W,X,Y,ee,te,se,ie,ne,oe,ae=e=>e;const le=/^[A-Za-z0-9\-\_]{40,50}\.[A-Za-z0-9\_\-]{40,50}$/,re=Object(n.a)(q||(q=ae`
<svg xmlns="http://www.w3.org/2000/svg" width="12" height="12" fill="currentColor" class="bi bi-chevron-down" viewBox="0 0 16 16">
  <path fill-rule="evenodd" d="M1.646 4.646a.5.5 0 0 1 .708 0L8 10.293l5.646-5.647a.5.5 0 0 1 .708.708l-6 6a.5.5 0 0 1-.708 0l-6-6a.5.5 0 0 1 0-.708z"/>
</svg>
`)),pe=Object(n.a)(H||(H=ae`
<svg width="12" height="12" fill="currentColor" viewBox="0 0 16 16">
  <path fill-rule="evenodd" d="M4.646 1.646a.5.5 0 0 1 .708 0l6 6a.5.5 0 0 1 0 .708l-6 6a.5.5 0 0 1-.708-.708L10.293 8 4.646 2.354a.5.5 0 0 1 0-.708z"/>
</svg>
`));t.default=class extends a.a{renderView(){const e=(this.props.node||"").split("/"),t=e.length&&e[0];let s=o.a.public;if("Local"===t)s=o.a.local;else if("Group"===t){s=o.a.group(e.length>=2&&e[1]||void 0)}const i=!e[0].length,a=e.map(((t,s)=>{const i=(t=decodeURIComponent(t)).substr(1),o=i.match(le);return Object(n.a)(Q||(Q=ae`
        ${0} <a href="/explorer/${0}">
            ${0}
        </a>
        ${0}
      `),pe,encodeURIComponent(e.slice(0,s+1).join("/")),o?Object(n.a)(W||(W=ae`<${0} key=${0} pub=${0} placeholder="profile name" />`),h.a,i,i):t,o?Object(n.a)(X||(X=ae`<small> (<a href="/profile/${0}">${0}</a>)</small>`),i,Object(V.c)("profile")):"")})),l=this.state;return Object(n.a)(Y||(Y=ae`
      <p>
        <a href="/explorer">All</a> ${0}
        ${0}
      </p>
      ${0}
    `),e[0].length?a:"",i?Object(n.a)(ee||(ee=ae`<small class="mar-left5">Iris raw data.</small>`)):"",i?Object(n.a)(te||(te=ae`
        <div class="explorer-row">
            <span onClick=${0}>${0}</span>
            <a href="/explorer/Public"><b>Public</b></a>
            <small class="mar-left5">(synced with peers)</small>
        </div>
        ${0}
        <div class="explorer-row">
            <span onClick=${0}>${0}</span>
            <a href="/explorer/Group"><b>Group</b></a>
            <small class="mar-left5">(public data, composite object of all the users in the <a href="/explorer/Local%2Fgroups">group</a>)</small>
        </div>
        ${0}
        <div class="explorer-row">
            <span onClick=${0}>${0}</span>
            <a href="/explorer/Local"><b>Local</b></a>
            <small class="mar-left5">(only stored on your device)</small>
        </div>
        ${0}
      `),(()=>this.setState({publicOpen:!l.publicOpen})),l.publicOpen?re:pe,l.publicOpen?Object(n.a)(se||(se=ae`<${0} indent=${0} gun=${0} key='Public' path='Public'/>`),J,1,o.a.public):"",(()=>this.setState({groupOpen:!l.groupOpen})),l.groupOpen?re:pe,l.groupOpen?Object(n.a)(ie||(ie=ae`<${0} indent=${0} gun=${0} key='Group' path='Group'/>`),J,1,o.a.public):"",(()=>this.setState({localOpen:!l.localOpen})),l.localOpen?re:pe,l.localOpen?Object(n.a)(ne||(ne=ae`<${0} indent=${0} gun=${0} key="Local" path='Local'/>`),J,1,o.a.local):""):Object(n.a)(oe||(oe=ae`
        <${0} indent=${0} showTools=${0} gun=${0} key=${0} path=${0}/>
      `),J,0,!0,s,this.props.node,this.props.node))}}}}]);
//# sourceMappingURL=2.chunk.1117a.esm.js.map