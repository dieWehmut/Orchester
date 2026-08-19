import {
  createMemoryHistory,
  createRouter,
  createWebHistory,
  type Router,
} from 'vue-router'

export function createAppRouter(mode: 'web' | 'memory' = 'web'): Router {
  const history = mode === 'memory' ? createMemoryHistory() : createWebHistory(import.meta.env.BASE_URL)

  return createRouter({
    history,
    routes: [
      {
        path: '/',
        redirect: { name: 'workspace' },
      },
      {
        path: '/workspace',
        name: 'workspace',
        component: () => import('./views/WorkspaceView.vue'),
        meta: { titleKey: 'routes.workspace' },
      },
      {
        path: '/settings',
        name: 'settings',
        component: () => import('./views/SettingsView.vue'),
        meta: { titleKey: 'routes.settings' },
      },
      {
        path: '/:pathMatch(.*)*',
        name: 'not-found',
        component: () => import('./views/NotFoundView.vue'),
        meta: { titleKey: 'routes.notFound' },
      },
    ],
  })
}

export const appRouter = createAppRouter()
