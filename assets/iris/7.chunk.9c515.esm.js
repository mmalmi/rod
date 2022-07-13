(window.webpackJsonp=window.webpackJsonp||[]).push([[7,9],{UIeC:function(t,e,s){"use strict";function a(){return a=Object.assign||function(t){for(var e=1;e<arguments.length;e++){var s=arguments[e];for(var a in s)Object.prototype.hasOwnProperty.call(s,a)&&(t[a]=s[a])}return t},a.apply(this,arguments)}s.r(e);var i=s("4Iz4"),r=s("3rgF"),c=s("lBHI"),o=s("jMw0"),l=s("jg5f"),n=s("Y3FI"),p=s("L8Yj"),d=s("vI8o"),h=s("rUzK"),u=s("d17u"),$=s("yqR5"),b=s("kv13"),v=s("DrMS"),m=s("3kvY");let f,g,j,y,O,C,w,x,k,I,P,M,S,_,U,D=t=>t;e.default=class extends v.a{constructor(){super(),this.followedUsers=new Set,this.followers=new Set,this.cart={},this.carts={},this.state={items:{}},this.id="profile",this.class="public-messages-view"}addToCart(t,e,s){s.stopPropagation();const a=(this.cart[t+e]||0)+1;c.a.local.get("cart").get(e).get(t).put(a)}renderUserStore(t){const e=o.a.channels[t],s=e&&e.uuid,a=!(this.isMyProfile||t.length<40);let c;return c=this.isMyProfile?Object(i.a)(f||(f=D`<${0} currentPhoto=${0} placeholder=${0} callback=${0}/>`),l.a,this.state.photo,t,(t=>this.onProfilePhotoSet(t))):this.state.photo?Object(i.a)(g||(g=D`<${0} class="profile-photo" src=${0}/>`),p.a,this.state.photo):Object(i.a)(j||(j=D`<${0} str=${0} width=250/>`),b.a,t),Object(i.a)(y||(y=D`
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
    `),c,d.a,Object(r.c)("profile_name"),Object(r.c)("name"),t,d.a,Object(r.c)("store_description"),t,t,this.state.followedUserCount,Object(r.c)("following"),t,this.state.followerCount,Object(r.c)("followers"),this.followedUsers.has(o.a.getPubKey())?Object(i.a)(O||(O=D`
                  <p><small>${0}</small></p>
                `),Object(r.c)("follows_you")):"",a?Object(i.a)(C||(C=D`<${0} id=${0}/>`),$.a,t):"",(()=>Object(n.route)(`/chat/${t}`)),Object(r.c)("send_message"),s?"":Object(i.a)(w||(w=D`
                  <${0} text=${0} title=${0} copyStr=${0}/>
                `),u.a,Object(r.c)("copy_link"),this.state.name,window.location.href),this.isMyProfile?Object(r.c)("about"):"",this.isMyProfile,(t=>this.onAboutInput(t)),this.state.about,Object(r.c)("store"),this.renderItems())}renderItems(){const t=Object.keys(this.cart).reduce(((t,e)=>t+this.cart[e]),0),e=Object.keys(this.state.items);return Object(i.a)(x||(x=D`
      ${0}
      ${0}
      <div class="thumbnail-items">
         ${0}
        ${0}
        ${0}
      </div>
    `),this.props.store||this.state.noFollows?"":Object(i.a)(k||(k=D`<${0}/>`),h.a),t?Object(i.a)(I||(I=D`
        <p>
          <button onClick=${0}>${0}(${0})</button>
        </p>
      `),(()=>Object(n.route)("/checkout")),Object(r.c)("shopping_cart"),t):"",this.isMyProfile?Object(i.a)(P||(P=D`
          <div class="thumbnail-item store-item" onClick=${0}>
            <a href="/product/new" class="name">${0}</a>
          </div>
        `),(()=>Object(n.route)("/product/new")),Object(r.c)("add_item")):"",e.length?"":Object(i.a)(M||(M=D`<p> ${0}</p>`),Object(r.c)("no_items_to_show")),e.map((t=>{const e=this.state.items[t];return Object(i.a)(S||(S=D`
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
          `),(()=>Object(n.route)(`/product/${t}/${e.from}`)),p.a,e.photo||"",t,e.from||this.props.store,e.name,this.props.store?"":Object(i.a)(_||(_=D`
                <small>by <${0} path="profile/name" editable="false" placeholder="Name" user=${0}/></small>
              `),d.a,e.from),e.description,e.price,(s=>this.addToCart(t,e.from,s)),Object(r.c)("add_to_cart"),this.cart[t]?` (${this.cart[t]})`:"")})))}renderView(){return this.props.store?this.renderUserStore(this.props.store):Object(i.a)(U||(U=D`
      <p dangerouslySetInnerHTML=${0}></p>
      <${0} />
      ${0}
    `),{__html:Object(r.c)("this_is_a_prototype_store",`href="/store/${o.a.getPubKey()}"`)},m.a,this.renderItems())}updateTotalPrice(){const t=Object.keys(this.cart).reduce(((t,e)=>{const s=this.state.items[e];return t+(s&&parseInt(s.price)||0)*this.cart[e]}),0);this.setState({totalPrice:t})}componentDidUpdate(t){t.store!==this.props.store&&this.componentDidMount()}getCartFromUser(t){c.a.local.get("cart").get(t).map(this.sub(((e,s)=>{"#"!==s&&(this.cart[s+t]=e,this.carts[t]=this.carts[t]||{},this.carts[t][s]=e,this.setState({cart:this.cart,carts:this.carts}),this.updateTotalPrice())}),`cart${t}`))}onProduct(t,e,s,i,r){this.eventListeners[`products${r}`]=i;const c=this.state.items;if(t&&"object"==typeof t){const s={};t.from=r,s[e]=t,a(c,s),this.updateTotalPrice()}else delete c[e];this.setState({items:c})}getProductsFromUser(t){c.a.public.user(t).get("store").get("products").map().on(this.sub(((...e)=>this.onProduct(...e,t)),`${t}products`))}getAllCarts(){const t={};c.a.local.get("cart").map(this.sub(((e,s)=>{s?t[s]||(t[s]=!0,this.getCartFromUser(s)):delete t[s]})))}getAllProducts(t){c.a.group(t).map("store/products",this.sub(((...t)=>{this.onProduct(...t)})))}componentDidMount(){const t=this.props.store;if(Object.values(this.eventListeners).forEach((t=>t.off())),this.cart={},this.isMyProfile=o.a.getPubKey()===t,t)this.getCartFromUser(t),this.getProductsFromUser(t);else{let t;c.a.local.get("filters").get("group").on(this.sub((e=>{e&&e!==t&&(t=e,this.getAllProducts(e))}))),this.getAllCarts()}}}},x8GX:function(t,e,s){"use strict";s.r(e);var a=s("4Iz4"),i=s("lBHI"),r=s("jMw0"),c=s("Y3FI"),o=s("L8Yj"),l=s("UIeC"),n=s("3rgF"),p=s("vI8o");let d,h,u,$,b,v,m,f,g,j=t=>t;e.default=class extends l.default{constructor(){super(),this.followedUsers=new Set,this.followers=new Set,this.state.paymentMethod="bitcoin",this.state.delivery={}}changeItemCount(t,e){this.cart[t]=Math.max(this.cart[t]+e,0),i.a.local.get("cart").get(this.props.store).get(t).put(this.cart[t])}confirm(){const t=this.props.store;r.a.newChannel(t);const e={};Object.keys(this.cart).forEach((t=>{const s=this.cart[t];s&&(e[t]=s)})),r.a.channels[t].send({text:`New order: ${JSON.stringify(e)}, delivery: ${JSON.stringify(this.state.delivery)}, payment: ${this.state.paymentMethod}`,order:!0}),i.a.local.get("cart").get(t).map(((e,s)=>{e&&i.a.local.get("cart").get(t).get(s).put(null)})),Object(c.route)(`/chat/${t}`)}renderCart(){return Object(a.a)(d||(d=j`
      <h3 class="side-padding-xs">${0}</h3>
      <div class="flex-table">
        ${0}
        <div class="flex-row">
          <div class="flex-cell"></div>
          <div class="flex-cell no-flex"><b>${0} ${0} €</b></div>
        </div>
      </div>
      <p class="side-padding-xs">
        <button onClick=${0}>${0}</button>
      </p>
    `),Object(n.c)("shopping_cart"),Object.keys(this.cart).filter((t=>!!this.cart[t]&&!!this.state.items[t])).map((t=>{const e=this.state.items[t];return Object(a.a)(h||(h=j`
            <div class="flex-row">
              <div class="flex-cell">
                <a href=${0}>
                  <${0} src=${0}/>
                  ${0}
                </a>
              </div>
              <div class="flex-cell no-flex price-cell">
                <p>
                  <span class="unit-price">${0} €</span>
                  <button onClick=${0}>-</button>
                  <input type="text" value=${0} onInput=${0}/>
                  <button onClick=${0}>+</button>
                </p>
                <span class="price">${0} €</span>
              </div>
            </div>
          `),`/product/${t}/${this.props.store}`,o.a,e.thumbnail,e.name||"item",parseInt(e.price),(()=>this.changeItemCount(t,-1)),this.cart[t],(()=>this.changeItemCount(t,null)),(()=>this.changeItemCount(t,1)),parseInt(e.price)*this.cart[t])})),Object(n.c)("total"),this.state.totalPrice,(()=>this.setState({page:"delivery"})),Object(n.c)("next"))}renderDelivery(){return Object(a.a)(u||(u=j`
      <div class="side-padding-xs">
        <h3>${0}</h3>
        <p>
          <input type="text" placeholder=${0} value=${0} onInput=${0}/>
        </p>
        <p>
          <input type="text" placeholder=${0} value=${0} onInput=${0}/>
        </p>
        <p>
          <input type="text" placeholder=${0} value=${0} onInput=${0}/>
        </p>
        <button onClick=${0}>${0}</button>
      </div>
    `),Object(n.c)("delivery"),Object(n.c)("name"),this.state.delivery.name,(t=>i.a.local.get("delivery").get("name").put(t.target.value)),Object(n.c)("address"),this.state.delivery.address,(t=>i.a.local.get("delivery").get("address").put(t.target.value)),Object(n.c)("email_optional"),this.state.delivery.email,(t=>i.a.local.get("delivery").get("email").put(t.target.value)),(()=>this.setState({page:"payment"})),Object(n.c)("next"))}paymentMethodChanged(t){const e=t.target.firstChild&&t.target.firstChild.value;e&&i.a.local.get("paymentMethod").put(e)}renderPayment(){return Object(a.a)($||($=j`
      <div class="side-padding-xs">
        <h3>${0}:</h3>
        <p>
          <label for="bitcoin" onClick=${0}>
            <input type="radio" name="payment" id="bitcoin" value="bitcoin" checked=${0}/>
            Bitcoin
          </label>
        </p>
        <p>
          <label for="dogecoin" onClick=${0}>
            <input type="radio" name="payment" id="dogecoin" value="dogecoin" checked=${0}/>
            Dogecoin
          </label>
        </p>
        <button onClick=${0}>${0}</button>
      </div>
    `),Object(n.c)("payment_method"),(t=>this.paymentMethodChanged(t)),"bitcoin"===this.state.paymentMethod,(t=>this.paymentMethodChanged(t)),"dogecoin"===this.state.paymentMethod,(()=>this.setState({page:"confirmation"})),Object(n.c)("next"))}renderConfirmation(){return Object(a.a)(b||(b=j`
      <h3 class="side-padding-xs">${0}</h3>
      <div class="flex-table">
        ${0}
        <div class="flex-row">
          <div class="flex-cell"></div>
          <div class="flex-cell no-flex"><b>${0} ${0} €</b></div>
        </div>
      </div>
      <p>
      ${0}:<br/>
        ${0}<br/>
        ${0}<br/>
        ${0}
      </p>
      <p>${0}: <b>${0}</b></p>
      <p class="side-padding-xs"><button onClick=${0}>${0}</button></p>
    `),Object(n.c)("confirm"),Object.keys(this.cart).filter((t=>!!this.cart[t]&&!!this.state.items[t])).map((t=>{const e=this.state.items[t];return Object(a.a)(v||(v=j`
            <div class="flex-row">
              <div class="flex-cell">
                <${0} src=${0}/>
                ${0}
              </div>
              <div class="flex-cell no-flex price-cell">
                <p>
                  ${0} x ${0} €
                </p>
                <span class="price">${0} €</span>
              </div>
            </div>
          `),o.a,e.thumbnail,e.name||"item",this.cart[t],parseInt(e.price),parseInt(e.price)*this.cart[t])})),Object(n.c)("total"),this.state.totalPrice,Object(n.c)("delivery"),this.state.delivery.name,this.state.delivery.address,this.state.delivery.email,Object(n.c)("payment_method"),this.state.paymentMethod,(()=>this.confirm()),Object(n.c)("confirm_button"))}renderCartList(){return Object(a.a)(m||(m=j`
    <div class="main-view" id="profile">
      <div class="content">
        <h2>${0}</h2>
        ${0}
      </div>
    </div>`),Object(n.c)("shopping_carts"),this.state.carts&&Object.keys(this.state.carts).map((t=>{const e=Object.keys(this.state.carts[t]).reduce(((e,s)=>e+this.state.carts[t][s]),0);if(e)return Object(a.a)(f||(f=j`
            <p>
              <a href="/checkout/${0}">
                <${0} path= ${0} user=${0} editable="false"/> (${0})
              </a>
            </p>
          `),t,p.a,Object(n.c)("profile_name"),t,e)})))}render(){if(!this.props.store)return this.renderCartList();let t;const e=this.state.page;return t="delivery"===e?this.renderDelivery():"confirmation"===e?this.renderConfirmation():"payment"===e?this.renderPayment():this.renderCart(),Object(a.a)(g||(g=j`
    <div class="main-view" id="profile">
      <div class="content">
        <p>
          <a href="/store/${0}"><${0} path="profile/name" user=${0}/></a>
        </p>
        <div id="store-steps">
          <div class=${0} onClick=${0}>${0}</div>
          <div class=${0} onClick=${0}>${0}</div>
          <div class=${0} onClick=${0}>${0}</div>
          <div class=${0} onClick=${0}>${0}</div>
        </div>
        ${0}
      </div>
    </div>`),this.props.store,p.a,this.props.store,"cart"===e?"active":"",(()=>this.setState({page:"cart"})),Object(n.c)("shopping_cart"),"delivery"===e?"active":"",(()=>this.setState({page:"delivery"})),Object(n.c)("delivery"),"payment"===e?"active":"",(()=>this.setState({page:"payment"})),Object(n.c)("payment"),"confirmation"===e?"active":"",(()=>this.setState({page:"confirmation"})),Object(n.c)("confirm"),t)}componentDidUpdate(t){t.store!==this.props.store&&this.componentDidMount()}componentDidMount(){l.default.prototype.componentDidMount.call(this),Object.values(this.eventListeners).forEach((t=>t.off())),this.eventListeners=[];const t=this.props.store;this.carts={},t?(this.setState({page:"cart"}),i.a.local.get("cart").get(t).map(((t,e)=>{this.cart[e]=t,this.setState({cart:this.cart})})),i.a.local.get("paymentMethod").on((t=>this.setState({paymentMethod:t}))),i.a.local.get("delivery").open((t=>this.setState({delivery:t})))):this.getAllCarts()}}}}]);
//# sourceMappingURL=7.chunk.9c515.esm.js.map