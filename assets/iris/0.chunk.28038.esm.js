(window.webpackJsonp=window.webpackJsonp||[]).push([[0,9],{UIeC:function(t,e,s){"use strict";function o(){return o=Object.assign||function(t){for(var e=1;e<arguments.length;e++){var s=arguments[e];for(var o in s)Object.prototype.hasOwnProperty.call(s,o)&&(t[o]=s[o])}return t},o.apply(this,arguments)}s.r(e);var r=s("4Iz4"),i=s("3rgF"),c=s("lBHI"),a=s("jMw0"),p=s("jg5f"),n=s("Y3FI"),l=s("L8Yj"),h=s("vI8o"),d=s("rUzK"),u=s("d17u"),$=s("yqR5"),b=s("kv13"),m=s("DrMS"),f=s("3kvY");let j,O,g,v,w,P,y,C,_,I,k,M,U,S,D,F=t=>t;e.default=class extends m.a{constructor(){super(),this.followedUsers=new Set,this.followers=new Set,this.cart={},this.carts={},this.state={items:{}},this.id="profile",this.class="public-messages-view"}addToCart(t,e,s){s.stopPropagation();const o=(this.cart[t+e]||0)+1;c.a.local.get("cart").get(e).get(t).put(o)}renderUserStore(t){const e=a.a.channels[t],s=e&&e.uuid,o=!(this.isMyProfile||t.length<40);let c;return c=this.isMyProfile?Object(r.a)(j||(j=F`<${0} currentPhoto=${0} placeholder=${0} callback=${0}/>`),p.a,this.state.photo,t,(t=>this.onProfilePhotoSet(t))):this.state.photo?Object(r.a)(O||(O=F`<${0} class="profile-photo" src=${0}/>`),l.a,this.state.photo):Object(r.a)(g||(g=F`<${0} str=${0} width=250/>`),b.a,t),Object(r.a)(v||(v=F`
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
    `),c,h.a,Object(i.c)("profile_name"),Object(i.c)("name"),t,h.a,Object(i.c)("store_description"),t,t,this.state.followedUserCount,Object(i.c)("following"),t,this.state.followerCount,Object(i.c)("followers"),this.followedUsers.has(a.a.getPubKey())?Object(r.a)(w||(w=F`
                  <p><small>${0}</small></p>
                `),Object(i.c)("follows_you")):"",o?Object(r.a)(P||(P=F`<${0} id=${0}/>`),$.a,t):"",(()=>Object(n.route)(`/chat/${t}`)),Object(i.c)("send_message"),s?"":Object(r.a)(y||(y=F`
                  <${0} text=${0} title=${0} copyStr=${0}/>
                `),u.a,Object(i.c)("copy_link"),this.state.name,window.location.href),this.isMyProfile?Object(i.c)("about"):"",this.isMyProfile,(t=>this.onAboutInput(t)),this.state.about,Object(i.c)("store"),this.renderItems())}renderItems(){const t=Object.keys(this.cart).reduce(((t,e)=>t+this.cart[e]),0),e=Object.keys(this.state.items);return Object(r.a)(C||(C=F`
      ${0}
      ${0}
      <div class="thumbnail-items">
         ${0}
        ${0}
        ${0}
      </div>
    `),this.props.store||this.state.noFollows?"":Object(r.a)(_||(_=F`<${0}/>`),d.a),t?Object(r.a)(I||(I=F`
        <p>
          <button onClick=${0}>${0}(${0})</button>
        </p>
      `),(()=>Object(n.route)("/checkout")),Object(i.c)("shopping_cart"),t):"",this.isMyProfile?Object(r.a)(k||(k=F`
          <div class="thumbnail-item store-item" onClick=${0}>
            <a href="/product/new" class="name">${0}</a>
          </div>
        `),(()=>Object(n.route)("/product/new")),Object(i.c)("add_item")):"",e.length?"":Object(r.a)(M||(M=F`<p> ${0}</p>`),Object(i.c)("no_items_to_show")),e.map((t=>{const e=this.state.items[t];return Object(r.a)(U||(U=F`
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
          `),(()=>Object(n.route)(`/product/${t}/${e.from}`)),l.a,e.photo||"",t,e.from||this.props.store,e.name,this.props.store?"":Object(r.a)(S||(S=F`
                <small>by <${0} path="profile/name" editable="false" placeholder="Name" user=${0}/></small>
              `),h.a,e.from),e.description,e.price,(s=>this.addToCart(t,e.from,s)),Object(i.c)("add_to_cart"),this.cart[t]?` (${this.cart[t]})`:"")})))}renderView(){return this.props.store?this.renderUserStore(this.props.store):Object(r.a)(D||(D=F`
      <p dangerouslySetInnerHTML=${0}></p>
      <${0} />
      ${0}
    `),{__html:Object(i.c)("this_is_a_prototype_store",`href="/store/${a.a.getPubKey()}"`)},f.a,this.renderItems())}updateTotalPrice(){const t=Object.keys(this.cart).reduce(((t,e)=>{const s=this.state.items[e];return t+(s&&parseInt(s.price)||0)*this.cart[e]}),0);this.setState({totalPrice:t})}componentDidUpdate(t){t.store!==this.props.store&&this.componentDidMount()}getCartFromUser(t){c.a.local.get("cart").get(t).map(this.sub(((e,s)=>{"#"!==s&&(this.cart[s+t]=e,this.carts[t]=this.carts[t]||{},this.carts[t][s]=e,this.setState({cart:this.cart,carts:this.carts}),this.updateTotalPrice())}),`cart${t}`))}onProduct(t,e,s,r,i){this.eventListeners[`products${i}`]=r;const c=this.state.items;if(t&&"object"==typeof t){const s={};t.from=i,s[e]=t,o(c,s),this.updateTotalPrice()}else delete c[e];this.setState({items:c})}getProductsFromUser(t){c.a.public.user(t).get("store").get("products").map().on(this.sub(((...e)=>this.onProduct(...e,t)),`${t}products`))}getAllCarts(){const t={};c.a.local.get("cart").map(this.sub(((e,s)=>{s?t[s]||(t[s]=!0,this.getCartFromUser(s)):delete t[s]})))}getAllProducts(t){c.a.group(t).map("store/products",this.sub(((...t)=>{this.onProduct(...t)})))}componentDidMount(){const t=this.props.store;if(Object.values(this.eventListeners).forEach((t=>t.off())),this.cart={},this.isMyProfile=a.a.getPubKey()===t,t)this.getCartFromUser(t),this.getProductsFromUser(t);else{let t;c.a.local.get("filters").get("group").on(this.sub((e=>{e&&e!==t&&(t=e,this.getAllProducts(e))}))),this.getAllCarts()}}}},YVdf:function(t,e,s){"use strict";s.r(e);var o=s("4Iz4"),r=s("lBHI"),i=s("jMw0"),c=s("3rgF"),a=s("Y3FI"),p=s("UIeC"),n=s("vI8o");let l,h,d,u,$,b,m=t=>t;e.default=class extends p.default{constructor(){super(),this.followedUsers=new Set,this.followers=new Set}addToCart(){const t=(this.cart[this.props.product]||0)+1;r.a.local.get("cart").get(this.props.store).get(this.props.product).put(t)}newProduct(){return Object(o.a)(l||(l=m`
      <div class="main-view" id="profile">
        <div class="content">
          <a href="/store/${0}"><${0} path="profile/name" placeholder=${0}  user=${0} /></a>
          <h3> ${0}</h3>
          <h2 contenteditable placeholder=${0} onInput=${0} />
          <textarea placeholder=${0} onInput=${0} style="resize: vertical"/>
          <input type="number" placeholder=${0} onInput=${0}/>
          <hr/>
          <p>
            ${0}:
          </p>
          <p>
            <input placeholder=${0} onInput=${0} />
          </p>
          <button onClick=${0}>${0}</button>
        </div>
      </div>
    `),i.a.getPubKey(),n.a,Object(c.c)("name"),i.a.getPubKey(),Object(c.c)("add_item"),Object(c.c)("item_id"),(t=>this.newProductName=t.target.innerText),Object(c.c)("item_description"),(t=>this.newProductDescription=t.target.value),Object(c.c)("price"),(t=>this.newProductPrice=parseInt(t.target.value)),Object(c.c)("item_id"),Object(c.c)("item_id"),(t=>this.newProductId=t.target.value),(t=>this.addItemClicked(t)),Object(c.c)("add_item"))}onClickDelete(){confirm("Delete product? This cannot be undone.")&&(r.a.public.user().get("store").get("products").get(this.props.product).put(null),Object(a.route)(`/store/${this.props.store}`))}showProduct(){const t=Object.values(this.cart).reduce(((t,e)=>t+e),0);return this.state.product?Object(o.a)(d||(d=m`
    <div class="main-view" id="profile">
      <div class="content">
        <a href="/store/${0}"><${0} editable="false" path="profile/name" user=${0}/></a>
        ${0}
        ${0}
      </div>
    </div>`),this.props.store,n.a,this.props.store,t?Object(o.a)(u||(u=m`
          <p>
            <button onClick=${0}>${0} (${0})</button>
          </p>
        `),(()=>Object(a.route)(`/checkout/${this.props.store}`)),Object(c.c)("shopping_cart"),t):"",this.state.product?Object(o.a)($||($=m`
          <${0} tag="h3" user=${0} path="store/products/${0}/name"/>
          <iris-img btn-class="btn btn-primary" user=${0} path="store/products/${0}/photo"/>
          <p class="description">
            <${0} user=${0} path="store/products/${0}/description"/>
          </p>
          <p class="price">
            <${0} placeholder=${0} user=${0} path="store/products/${0}/price"/>
          </p>
          <button class="add" onClick=${0}>
            ${0}
            ${0}
          </button>
          ${0}
        `),n.a,this.props.store,this.props.product,this.props.store,this.props.product,n.a,this.props.store,this.props.product,n.a,Object(c.c)("price"),this.props.store,this.props.product,(()=>this.addToCart()),Object(c.c)("add_to_cart"),this.cart[this.props.product]?` (${this.cart[this.props.product]})`:"",this.isMyProfile?Object(o.a)(b||(b=m`
            <p><button onClick=${0}>${0}</button></p>
          `),(t=>this.onClickDelete(t)),Object(c.c)("delete_item")):""):""):Object(o.a)(h||(h=m``))}render(){return this.props.store&&this.props.product?this.showProduct():this.newProduct()}componentDidUpdate(t){t.product!==this.props.product&&this.componentDidMount()}addItemClicked(){const t={name:this.newProductName,description:this.newProductDescription,price:this.newProductPrice};r.a.public.user().get("store").get("products").get(this.newProductId||this.newProductName).put(t),Object(a.route)(`/store/${i.a.getPubKey()}`)}componentDidMount(){p.default.prototype.componentDidMount.call(this);const t=this.props.store;this.setState({followedUserCount:0,followerCount:0,name:"",photo:"",about:""}),this.isMyProfile=i.a.getPubKey()===t,this.props.product&&t&&r.a.public.user(t).get("store").get("products").get(this.props.product).on((t=>this.setState({product:t})))}}}}]);
//# sourceMappingURL=0.chunk.28038.esm.js.map