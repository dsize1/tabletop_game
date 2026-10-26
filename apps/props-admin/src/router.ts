import { createRouter, createWebHistory } from "vue-router";
import Dashboard from "./pages/Dashboard.vue";
import ResourcePage from "./pages/ResourcePage.vue";
import CardRelation from "./pages/CardRelation.vue";
import ImportPage from "./pages/ImportPage.vue";

export const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    { path: "/", component: Dashboard },
    { path: "/packs", component: ResourcePage, meta: { table: "prop_packs" } },
    { path: "/identities", component: ResourcePage, meta: { table: "sgs_identities" } },
    { path: "/identity-setup", component: ResourcePage, meta: { table: "sgs_identity_setup" } },
    { path: "/card-defs", component: ResourcePage, meta: { table: "sgs_card_defs" } },
    { path: "/cards", component: ResourcePage, meta: { table: "sgs_cards" } },
    { path: "/generals", component: ResourcePage, meta: { table: "sgs_generals" } },
    { path: "/skills", component: ResourcePage, meta: { table: "sgs_skills" } },
    { path: "/general-skills", component: ResourcePage, meta: { table: "sgs_general_skills" } },
    { path: "/card-relation", component: CardRelation },
    { path: "/import", component: ImportPage },
  ],
});
