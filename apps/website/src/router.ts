import {
  createRouter,
  createWebHistory,
  type Router,
  type RouterHistory,
} from 'vue-router'

export function createWebsiteRouter(
  history: RouterHistory = createWebHistory(import.meta.env.BASE_URL),
): Router {
  return createRouter({
    history,
    routes: [
      {
        path: '/',
        name: 'home',
        component: () => import('./views/HomeView.vue'),
      },
      {
        path: '/architecture',
        name: 'architecture',
        component: () => import('./views/ArchitectureView.vue'),
      },
      {
        path: '/install',
        name: 'install',
        component: () => import('./views/InstallView.vue'),
      },
      {
        path: '/:pathMatch(.*)*',
        name: 'not-found',
        component: () => import('./views/NotFoundView.vue'),
      },
    ],
  })
}

export const appRouter = createWebsiteRouter()
