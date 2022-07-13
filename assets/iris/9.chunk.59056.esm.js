(window.webpackJsonp=window.webpackJsonp||[]).push([[9],{UIeC:function(t,e,s){"use strict";function o(){return o=Object.assign||function(t){for(var e=1;e<arguments.length;e++){var s=arguments[e];for(var o in s)Object.prototype.hasOwnProperty.call(s,o)&&(t[o]=s[o])}return t},o.apply(this,arguments)}s.r(e);var r=s("4Iz4"),a=s("3rgF"),i=s("lBHI"),c=s("jMw0"),l=s("jg5f"),n=s("Y3FI"),h=s("L8Yj"),p=s("vI8o"),d=s("rUzK"),u=s("d17u"),$=s("yqR5"),b=s("kv13"),f=s("DrMS"),m=s("3kvY");let j,O,g,v,w,P,y,_,k,C,U,I,M,S,F,T=t=>t;e.default=class extends f.a{constructor(){super(),this.followedUsers=new Set,this.followers=new Set,this.cart={},this.carts={},this.state={items:{}},this.id="profile",this.class="public-messages-view"}addToCart(t,e,s){s.stopPropagation();const o=(this.cart[t+e]||0)+1;i.a.local.get("cart").get(e).get(t).put(o)}renderUserStore(t){const e=c.a.channels[t],s=e&&e.uuid,o=!(this.isMyProfile||t.length<40);let i;return i=this.isMyProfile?Object(r.a)(j||(j=T`<${0} currentPhoto=${0} placeholder=${0} callback=${0}/>`),l.a,this.state.photo,t,(t=>this.onProfilePhotoSet(t))):this.state.photo?Object(r.a)(O||(O=T`<${0} class="profile-photo" src=${0}/>`),h.a,this.state.photo):Object(r.a)(g||(g=T`<${0} str=${0} width=250/>`),b.a,t),Object(r.a)(v||(v=T`
      <div class="content">
        <div class="profile-top">
          <div class="profile-header">
            <div class="profile-photo-container">
              ${0}
            </div>
            <div class="profile-header-stuff">
              <h3 class="profile-name"><${0} path= ${0} placeholder= ${0} user=${0}/></h3>
              <div class="profile-about hidden-xs">
                <p class="profile-about-content">
                  <${0} path="store/about" placeholder=${0} attr="about" user=${0}/>
                </p>
              </div>
              <div class="profile-actions">
                <div class="follow-count">
                  <a href="/follows/${0}">
                    <span>${0}</span> ${0}
                  </a>
                  <a href="/followers/${0}">
                    <span>${0}</span> ${0}
                  </a>
                </div>
                ${0}
                ${0}
                <button onClick=${0}>${0}</button>
                ${0}
              </div>
            </div>
          </div>
          <div class="profile-about visible-xs-flex">
            <p class="profile-about-content" placeholder=${0} contenteditable=${0} onInput=${0}>${0}</p>
          </div>
        </div>

        <h3>${0}</h3>
        ${0}
      </div>
    `),i,p.a,Object(a.c)("profile_name"),Object(a.c)("name"),t,p.a,Object(a.c)("store_description"),t,t,this.state.followedUserCount,Object(a.c)("following"),t,this.state.followerCount,Object(a.c)("followers"),this.followedUsers.has(c.a.getPubKey())?Object(r.a)(w||(w=T`
                  <p><small>${0}</small></p>
                `),Object(a.c)("follows_you")):"",o?Object(r.a)(P||(P=T`<${0} id=${0}/>`),$.a,t):"",(()=>Object(n.route)(`/chat/${t}`)),Object(a.c)("send_message"),s?"":Object(r.a)(y||(y=T`
                  <${0} text=${0} title=${0} copyStr=${0}/>
                `),u.a,Object(a.c)("copy_link"),this.state.name,window.location.href),this.isMyProfile?Object(a.c)("about"):"",this.isMyProfile,(t=>this.onAboutInput(t)),this.state.about,Object(a.c)("store"),this.renderItems())}renderItems(){const t=Object.keys(this.cart).reduce(((t,e)=>t+this.cart[e]),0),e=Object.keys(this.state.items);return Object(r.a)(_||(_=T`
      ${0}
      ${0}
      <div class="thumbnail-items">
         ${0}
        ${0}
        ${0}
      </div>
    `),this.props.store||this.state.noFollows?"":Object(r.a)(k||(k=T`<${0}/>`),d.a),t?Object(r.a)(C||(C=T`
        <p>
          <button onClick=${0}>${0}(${0})</button>
        </p>
      `),(()=>Object(n.route)("/checkout")),Object(a.c)("shopping_cart"),t):"",this.isMyProfile?Object(r.a)(U||(U=T`
          <div class="thumbnail-item store-item" onClick=${0}>
            <a href="/product/new" class="name">${0}</a>
          </div>
        `),(()=>Object(n.route)("/product/new")),Object(a.c)("add_item")):"",e.length?"":Object(r.a)(I||(I=T`<p> ${0}</p>`),Object(a.c)("no_items_to_show")),e.map((t=>{const e=this.state.items[t];return Object(r.a)(M||(M=T`
            <div class="thumbnail-item store-item" onClick=${0}>
              <${0} src=${0}/>
              <a href="/product/${0}/${0}" class="name">${0}</a>
              ${0}
              <p class="description">${0}</p>
              <p class="price">${0}</p>
              <button class="add" onClick=${0}>
              ${0}
                ${0}
              </button>
            </div>
          `),(()=>Object(n.route)(`/product/${t}/${e.from}`)),h.a,e.photo||"",t,e.from||this.props.store,e.name,this.props.store?"":Object(r.a)(S||(S=T`
                <small>by <${0} path="profile/name" editable="false" placeholder="Name" user=${0}/></small>
              `),p.a,e.from),e.description,e.price,(s=>this.addToCart(t,e.from,s)),Object(a.c)("add_to_cart"),this.cart[t]?` (${this.cart[t]})`:"")})))}renderView(){return this.props.store?this.renderUserStore(this.props.store):Object(r.a)(F||(F=T`
      <p dangerouslySetInnerHTML=${0}></p>
      <${0} />
      ${0}
    `),{__html:Object(a.c)("this_is_a_prototype_store",`href="/store/${c.a.getPubKey()}"`)},m.a,this.renderItems())}updateTotalPrice(){const t=Object.keys(this.cart).reduce(((t,e)=>{const s=this.state.items[e];return t+(s&&parseInt(s.price)||0)*this.cart[e]}),0);this.setState({totalPrice:t})}componentDidUpdate(t){t.store!==this.props.store&&this.componentDidMount()}getCartFromUser(t){i.a.local.get("cart").get(t).map(this.sub(((e,s)=>{"#"!==s&&(this.cart[s+t]=e,this.carts[t]=this.carts[t]||{},this.carts[t][s]=e,this.setState({cart:this.cart,carts:this.carts}),this.updateTotalPrice())}),`cart${t}`))}onProduct(t,e,s,r,a){this.eventListeners[`products${a}`]=r;const i=this.state.items;if(t&&"object"==typeof t){const s={};t.from=a,s[e]=t,o(i,s),this.updateTotalPrice()}else delete i[e];this.setState({items:i})}getProductsFromUser(t){i.a.public.user(t).get("store").get("products").map().on(this.sub(((...e)=>this.onProduct(...e,t)),`${t}products`))}getAllCarts(){const t={};i.a.local.get("cart").map(this.sub(((e,s)=>{s?t[s]||(t[s]=!0,this.getCartFromUser(s)):delete t[s]})))}getAllProducts(t){i.a.group(t).map("store/products",this.sub(((...t)=>{this.onProduct(...t)})))}componentDidMount(){const t=this.props.store;if(Object.values(this.eventListeners).forEach((t=>t.off())),this.cart={},this.isMyProfile=c.a.getPubKey()===t,t)this.getCartFromUser(t),this.getProductsFromUser(t);else{let t;i.a.local.get("filters").get("group").on(this.sub((e=>{e&&e!==t&&(t=e,this.getAllProducts(e))}))),this.getAllCarts()}}}}}]);
//# sourceMappingURL=9.chunk.59056.esm.js.map