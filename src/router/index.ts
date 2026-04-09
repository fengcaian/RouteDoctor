import { createRouter, createWebHistory } from 'vue-router'

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/',
      name: 'dashboard',
      component: () => import('@/views/Dashboard.vue')
    },
    {
      path: '/ping',
      name: 'ping',
      component: () => import('@/views/PingView.vue')
    },
    {
      path: '/traceroute',
      name: 'traceroute',
      component: () => import('@/views/TraceView.vue')
    },
    {
      path: '/bandwidth',
      name: 'bandwidth',
      component: () => import('@/views/BandwidthView.vue')
    },
    {
      path: '/history',
      name: 'history',
      component: () => import('@/views/HistoryView.vue')
    }
  ]
})

export default router