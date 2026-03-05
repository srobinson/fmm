@Injectable()
export class UserService {
    findAll(): string[] {
        return [];
    }
}

@Controller('/users')
export class UserController {
    constructor(private readonly userService: UserService) {}

    getAll(): string[] {
        return this.userService.findAll();
    }
}

@Module({
    providers: [UserService],
    controllers: [UserController],
})
export class AppModule {}

export class PlainService {
    execute(): void {}
}
