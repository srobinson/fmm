
using System;
using System.Threading.Tasks;

namespace MyApp.Services
{
    public interface IUserService
    {
        Task<string> GetUserAsync(int id);
    }

    public class UserService : IUserService
    {
        public async Task<string> GetUserAsync(int id)
        {
            await Task.Delay(100);
            return $"User {id}";
        }

        public void Delete(int id) { }

        private bool Validate(int id) => id > 0;
    }

    internal class CacheHelper
    {
        internal void Clear() { }
    }
}
