import { defineStore } from 'pinia'
import { ref, watch } from 'vue'

export interface FavoriteTarget {
  id: string
  name: string        // 用户自定义名称（如"公司服务器"）
  target: string      // IP 或域名
  type: 'ping' | 'traceroute' | 'both'
  createdAt: number
}

export const useFavoritesStore = defineStore('favorites', () => {
  // 从 localStorage 加载
  const saved = localStorage.getItem('favorite-targets')
  const favorites = ref<FavoriteTarget[]>(saved ? JSON.parse(saved) : [])

  // 自动保存到 localStorage
  watch(favorites, (val) => {
    localStorage.setItem('favorite-targets', JSON.stringify(val))
  }, { deep: true })

  function addFavorite(name: string, target: string, type: FavoriteTarget['type'] = 'both') {
    // 检查是否已存在
    if (favorites.value.some(f => f.target === target)) return false

    favorites.value.push({
      id: crypto.randomUUID(),
      name,
      target,
      type,
      createdAt: Date.now()
    })
    return true
  }

  function removeFavorite(id: string) {
    const index = favorites.value.findIndex(f => f.id === id)
    if (index !== -1) {
      favorites.value.splice(index, 1)
    }
  }

  function updateFavorite(id: string, updates: Partial<Pick<FavoriteTarget, 'name' | 'target' | 'type'>>) {
    const fav = favorites.value.find(f => f.id === id)
    if (fav) {
      Object.assign(fav, updates)
    }
  }

  function isFavorite(target: string): boolean {
    return favorites.value.some(f => f.target === target)
  }

  function getFavoriteByTarget(target: string): FavoriteTarget | undefined {
    return favorites.value.find(f => f.target === target)
  }

  return {
    favorites,
    addFavorite,
    removeFavorite,
    updateFavorite,
    isFavorite,
    getFavoriteByTarget
  }
})
