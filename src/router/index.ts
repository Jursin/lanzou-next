import { createRouter, createWebHashHistory, type RouteRecordRaw } from 'vue-router'

const routes: RouteRecordRaw[] = [
  {
    path: '/',
    component: () => import('@/layouts/MainLayout.vue'),
    redirect: '/files',
    children: [
      { path: 'files', name: 'files', component: () => import('@/views/FilesView.vue'), meta: { title: '我的文件' } },
      { path: 'tasks', name: 'tasks', component: () => import('@/views/TasksView.vue'), meta: { title: '传输列表' } },
      { path: 'parse', name: 'parse', component: () => import('@/views/ParseView.vue'), meta: { title: '解析 URL' } },
      { path: 'setting', name: 'setting', component: () => import('@/views/SettingView.vue'), meta: { title: '设置' } },
    ],
  },
]

const router = createRouter({
  history: createWebHashHistory(),
  routes,
})

export default router
