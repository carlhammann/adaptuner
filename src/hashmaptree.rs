use std::collections::{HashMap, VecDeque};

#[derive(Debug)]
pub struct Node<K, V> {
    internal: V,
    pub parent: Option<K>,
    pub left_sibling: Option<K>,
    pub right_sibling: Option<K>,
    pub first_child: Option<K>,
    pub last_child: Option<K>,
}

impl<K, V> Node<K, V> {
    pub fn get(&self) -> &V {
        &self.internal
    }
    pub fn get_mut(&mut self) -> &mut V {
        &mut self.internal
    }
}

#[derive(Debug)]
pub struct HashMapTree<K, V> {
    internal: HashMap<K, Node<K, V>>,
}

struct SiblingsLeftToRight<'a, K, V> {
    next: Option<K>,
    tree: &'a HashMapTree<K, V>,
}

impl<'a, K, V> Iterator for SiblingsLeftToRight<'a, K, V>
where
    K: Eq + std::hash::Hash + Clone,
{
    type Item = K;
    fn next(&mut self) -> Option<Self::Item> {
        match &self.next {
            None { .. } => None,
            Some(i) => {
                let tmp = self.next.clone();
                self.next.clone_from(self.tree.right_sibling(&i));
                tmp
            }
        }
    }
}

struct SiblingsRightToLeft<'a, K, V> {
    next: Option<K>,
    tree: &'a HashMapTree<K, V>,
}

impl<'a, K, V> Iterator for SiblingsRightToLeft<'a, K, V>
where
    K: Eq + std::hash::Hash + Clone,
{
    type Item = K;
    fn next(&mut self) -> Option<Self::Item> {
        match &self.next {
            None { .. } => None,
            Some(i) => {
                let tmp = self.next.clone();
                self.next.clone_from(self.tree.left_sibling(&i));
                tmp
            }
        }
    }
}

struct Descendants<'a, K, V> {
    queue: VecDeque<K>,
    tree: &'a HashMapTree<K, V>,
}

impl<'a, K, V> Iterator for Descendants<'a, K, V>
where
    K: std::cmp::Eq + std::hash::Hash + Clone,
{
    type Item = K;
    fn next(&mut self) -> Option<Self::Item> {
        match self.queue.pop_front() {
            None { .. } => None,
            Some(k) => {
                let node = self.tree.get(&k).expect("Descendants: node not in tree");
                let mut push_if_some = |mk: Option<K>| match mk {
                    None { .. } => {}
                    Some(l) => self.queue.push_front(l),
                };
                push_if_some(node.right_sibling.clone());
                push_if_some(node.first_child.clone());
                Some(k)
            }
        }
    }
}

struct Ancestors<'a, K, V> {
    cur: Option<K>,
    tree: &'a HashMapTree<K, V>,
}

impl<'a, K, V> Iterator for Ancestors<'a, K, V>
where
    K: Eq + std::hash::Hash + Clone,
{
    type Item = K;
    fn next(&mut self) -> Option<Self::Item> {
        let old = self.cur.clone();
        match &old {
            None { .. } => {}
            Some(k) => {
                let node = self.tree.get(&k).expect("Ancestors: node not in tree");
                self.cur.clone_from(&node.parent);
            }
        }
        old
    }
}

struct DrainDescendants<'a, K, V> {
    queue: VecDeque<K>,
    tree: &'a mut HashMapTree<K, V>,
}

impl<'a, K, V> Iterator for DrainDescendants<'a, K, V>
where
    K: std::cmp::Eq + std::hash::Hash + Clone,
{
    type Item = (K, V);
    fn next(&mut self) -> Option<Self::Item> {
        match self.queue.pop_front() {
            None { .. } => None,
            Some(k) => {
                let node = self
                    .tree
                    .get(&k)
                    .expect("DrainDescendants: node not in tree");
                let mut push_if_some = |mk: Option<K>| match mk {
                    None { .. } => {}
                    Some(l) => self.queue.push_front(l),
                };
                push_if_some(node.right_sibling.clone());
                push_if_some(node.first_child.clone());
                let v = self.tree.internal.remove(&k);
                Some((k, v.unwrap().internal))
            }
        }
    }
}

impl<K, V> HashMapTree<K, V> {
    pub fn new() -> Self {
        let internal = HashMap::new();
        HashMapTree { internal }
    }

    pub fn get(&self, key: &K) -> Option<&Node<K, V>>
    where
        K: Eq + std::hash::Hash,
    {
        self.internal.get(key)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut Node<K, V>>
    where
        K: Eq + std::hash::Hash,
    {
        self.internal.get_mut(key)
    }

    pub fn add_node(&mut self, key: K, value: V)
    where
        K: Eq + std::hash::Hash,
    {
        self.internal.insert(
            key,
            Node {
                internal: value,
                parent: None,
                left_sibling: None,
                right_sibling: None,
                first_child: None,
                last_child: None,
            },
        );
    }
    
    pub fn add_child(&mut self, parent_key: &K, child_key: K, child: V)
    where
        K: Eq + std::hash::Hash + Clone,
    {
        let parent = self
            .get_mut(parent_key)
            .expect("add_child: parent not in tree");
        let left_sibling = match &parent.last_child {
            None { .. } => None,
            Some(k) => Some(k.clone()),
        };
        parent.last_child = Some(child_key.clone());
        match &left_sibling {
            None { .. } => {
                parent.first_child = Some(child_key.clone());
            }
            Some(k) => {
                let l = self
                    .get_mut(&k)
                    .expect("add_child: last child of parent not in tree");
                l.right_sibling = Some(child_key.clone());
            }
        }
        let child_node = Node {
            internal: child,
            parent: Some(parent_key.clone()),
            left_sibling,
            right_sibling: None,
            first_child: None,
            last_child: None,
        };
        self.internal.insert(child_key, child_node);
    }

    /// All immediate child nodes, from left to right.
    pub fn children(&self, parent: &K) -> impl Iterator<Item = K> + '_
    where
        K: Clone + Eq + std::hash::Hash,
    {
        SiblingsLeftToRight {
            next: self.first_child(parent).clone(),
            tree: self,
        }
    }

    /// Like `children`, but reverses the order.
    pub fn children_right_to_left(&self, parent: &K) -> impl Iterator<Item = K> + '_
    where
        K: Clone + Eq + std::hash::Hash,
    {
        SiblingsRightToLeft {
            next: self.last_child(parent).clone(),
            tree: self,
        }
    }

    /// returns an iterator of all nodes strictly below the node with the given id. This is
    /// a pre-order depth-first search.
    pub fn descendants(&self, root: K) -> impl Iterator<Item = K> + '_
    where
        K: Eq + std::hash::Hash + Clone,
    {
        let mut queue = VecDeque::new();
        match self.first_child(&root) {
            None { .. } => {}
            Some(i) => queue.push_back(i.clone()),
        }
        Descendants { queue, tree: self }
    }

    pub fn ancestors(&self, child: K) -> impl Iterator<Item = K> + '_
    where
        K: Eq + std::hash::Hash + Clone,
    {
        Ancestors {
            cur: Some(child),
            tree: self,
        }
    }

    /// Returns an iterator of all nodes strictly below the node with the given id. This is
    /// a pre-order depth-first search. Will remove all items it iterates through.
    pub fn drain_descendants(&mut self, root: K) -> impl Iterator<Item = (K, V)> + '_
    where
        K: Eq + std::hash::Hash + Clone,
    {
        let mut queue = VecDeque::new();
        match self.first_child(&root) {
            None { .. } => {}
            Some(i) => queue.push_back(i.clone()),
        }
        let root_node = self
            .get_mut(&root)
            .expect("drain_descendants: root node not in tree");
        root_node.first_child = None;
        root_node.last_child = None;
        DrainDescendants { queue, tree: self }
    }

    pub fn parent(&self, i: &K) -> &Option<K>
    where
        K: Eq + std::hash::Hash,
    {
        let node = self.get(i).expect("parent: node not in tree");
        &node.parent
    }

    pub fn first_child(&self, i: &K) -> &Option<K>
    where
        K: Eq + std::hash::Hash,
    {
        let node = self.get(i).expect("first_child: node not in tree");
        &node.first_child
    }

    pub fn last_child(&self, i: &K) -> &Option<K>
    where
        K: Eq + std::hash::Hash,
    {
        let node = self.get(i).expect("last_child: node not in tree");
        &node.last_child
    }

    pub fn left_sibling(&self, i: &K) -> &Option<K>
    where
        K: Eq + std::hash::Hash,
    {
        let node = self.get(i).expect("left_sibling: node not in tree");
        &node.left_sibling
    }

    pub fn right_sibling(&self, i: &K) -> &Option<K>
    where
        K: Eq + std::hash::Hash,
    {
        let node = self.get(i).expect("right_sibling: node not in tree");
        &node.right_sibling
    }

    pub fn leftmost_sibling(&self, i: &K) -> K
    where
        K: Eq + std::hash::Hash + Clone,
    {
        let mut res = i.clone();
        let mut ls = self.left_sibling(i);
        while ls.is_some() {
            res.clone_from(ls.as_ref().unwrap());
            ls = self.left_sibling(&res);
        }
        res
    }

    /// Remove a node. The children of the removed node will take its place between its siblings.
    /// Returns the removed value, if there was one.
    pub fn remove(&mut self, key: &K) -> Option<V>
    where
        K: Eq + std::hash::Hash + Clone,
    {
        match self.internal.remove(key) {
            None { .. } => None,
            Some(v) => {
                match v.left_sibling {
                    None { .. } => {
                        // the removed node is the first child... we have to update the parent
                        match v.parent {
                            None { .. } => {}
                            Some(ref i) => {
                                let p = self.get_mut(&i).expect("remove: parent not in tree");
                                if v.first_child.is_some() {
                                    p.first_child.clone_from(&v.first_child);
                                    let lc = self
                                        .get_mut(
                                            v.last_child
                                                .as_ref()
                                                .expect("remove: node has first but no last child"),
                                        )
                                        .expect("remove: last child not in tree");
                                    lc.right_sibling.clone_from(&v.right_sibling);
                                } else {
                                    p.first_child.clone_from(&v.right_sibling);
                                }
                            }
                        }
                    }
                    Some(ref i) => {
                        let ls = self.get_mut(&i).expect("remove: left sibling not in tree");
                        ls.right_sibling.clone_from(&v.first_child);
                        let lc = self
                            .get_mut(
                                v.last_child
                                    .as_ref()
                                    .expect("remove: node has first but no last child"),
                            )
                            .expect("remove: last child not in tree");
                        lc.right_sibling.clone_from(&v.right_sibling);
                    }
                }
                match v.right_sibling {
                    None { .. } => {
                        // the removed node is the last child... we have to update the parent
                        match v.parent {
                            None { .. } => {}
                            Some(ref i) => {
                                let p = self.get_mut(&i).expect("remove: parent not in tree");
                                if v.first_child.is_some() {
                                    p.last_child = v.last_child;
                                    let fc = self
                                        .get_mut(
                                            v.first_child
                                                .as_ref()
                                                .expect("remove: node has last but no first child"),
                                        )
                                        .expect("remove: first child not in tree");
                                    fc.left_sibling.clone_from(&v.left_sibling);
                                } else {
                                    p.last_child.clone_from(&v.left_sibling);
                                }
                            }
                        }
                    }
                    Some(i) => {
                        let rs = self.get_mut(&i).expect("remove: right sibling not in tree");
                        rs.left_sibling.clone_from(&v.last_child);
                        let fc = self
                            .get_mut(
                                v.first_child
                                    .as_ref()
                                    .expect("remove: node has last but no first child"),
                            )
                            .expect("remove: first child not in tree");
                        fc.left_sibling.clone_from(&v.left_sibling);
                    }
                }
                Some(v.internal)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;

    use super::*;

    fn mock_tree() -> HashMapTree<&'static str, u8> {
        let mut t = HashMapTree::new();
        t.add_node("n", 0);
        t.add_child(&"n", "n1", 1);
        t.add_child(&"n", "n2", 2);
        t.add_child(&"n", "n3", 3);
        t.add_child(&"n2", "n2_1", 4);
        t.add_child(&"n2", "n2_2", 5);
        t.add_child(&"n2_1", "n2_1_1", 6);
        t.add_child(&"n3", "n3_1", 7);
        t
    }

    #[test]
    fn test_descendants() {
        assert_eq!(
            vec!["n1", "n2", "n2_1", "n2_1_1", "n2_2", "n3", "n3_1"],
            mock_tree().descendants("n").collect::<Vec<_>>()
        );
        assert_eq!(
            vec!["n2_1", "n2_1_1", "n2_2"],
            mock_tree().descendants("n2").collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_ancestors() {
        assert_eq!(vec!["n"], mock_tree().ancestors("n").collect::<Vec<_>>());
        assert_eq!(
            vec!["n3_1", "n3", "n"],
            mock_tree().ancestors("n3_1").collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_drain_descendants() {
        let mut t = mock_tree();
        assert_eq!(
            vec!["n2_1", "n2_1_1", "n2_2"],
            t.drain_descendants("n2")
                .map(|(k, _v)| k)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            vec!["n1", "n2", "n3", "n3_1"],
            t.descendants("n").collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_remove() {
        let mut t = mock_tree();

        assert_eq!(None, t.remove(&"asdf"));

        assert_eq!(Some(2), t.remove(&"n2"));
        assert_eq!(
            vec!["n1", "n2_1", "n2_1_1", "n2_2", "n3", "n3_1"],
            t.descendants("n").collect::<Vec<_>>()
        );
        assert_eq!(
            vec!["n1", "n2_1", "n2_2", "n3"],
            t.children(&"n").collect::<Vec<_>>()
        );
        assert_eq!(vec!["n1", "n2_1", "n2_2", "n3"], {
            let mut children = t.children_right_to_left(&"n").collect::<Vec<_>>();
            children.reverse();
            children
        });
        assert_eq!(
            vec![&"n", &"n1", &"n2_1", &"n2_1_1", &"n2_2", &"n3", &"n3_1"],
            {
                let mut all_nodes = t.internal.iter().map(|(s, _)| s).collect::<Vec<_>>();
                all_nodes.sort();
                all_nodes
            }
        );

        assert_eq!(None, t.remove(&"n2"));

        assert_eq!(Some(4), t.remove(&"n2_1"));
        assert_eq!(
            vec!["n1", "n2_1_1", "n2_2", "n3", "n3_1"],
            t.descendants("n").collect::<Vec<_>>()
        );
        assert_eq!(
            vec!["n1", "n2_1_1", "n2_2", "n3"],
            t.children(&"n").collect::<Vec<_>>()
        );
        assert_eq!(vec!["n1", "n2_1_1", "n2_2", "n3"], {
            let mut children = t.children_right_to_left(&"n").collect::<Vec<_>>();
            children.reverse();
            children
        });
        assert_eq!(vec![&"n", &"n1", &"n2_1_1", &"n2_2", &"n3", &"n3_1"], {
            let mut all_nodes = t.internal.iter().map(|(s, _)| s).collect::<Vec<_>>();
            all_nodes.sort();
            all_nodes
        });
    }
}
