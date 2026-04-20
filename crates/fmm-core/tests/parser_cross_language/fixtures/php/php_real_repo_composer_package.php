<?php

namespace League\Container;

use League\Container\Definition\DefinitionInterface;

interface ContainerInterface
{
    public function get(string $id): mixed;
    public function has(string $id): bool;
}

trait ContainerAwareTrait
{
    protected ?ContainerInterface $container = null;

    public function setContainer(ContainerInterface $container): self
    {
        $this->container = $container;
        return $this;
    }

    public function getContainer(): ContainerInterface
    {
        return $this->container;
    }
}

class Container implements ContainerInterface
{
    use ContainerAwareTrait;

    public function get(string $id): mixed
    {
        return $this->resolve($id);
    }

    public function has(string $id): bool
    {
        return isset($this->definitions[$id]);
    }

    public function add(string $id, mixed $concrete = null): DefinitionInterface
    {
        return $this->definitions[$id] = new Definition($id, $concrete);
    }

    private function resolve(string $id): mixed
    {
        return $this->definitions[$id]->resolve();
    }
}
