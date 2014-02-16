import { createApp } from "vue";
import App from "./App.vue";
import router from "./router";
import { fluent, locale } from "./i18n";
import "./styles.css";

document.documentElement.lang = locale.value;
createApp(App).use(router).use(fluent).mount("#app");
